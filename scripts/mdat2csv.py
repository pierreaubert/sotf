#!/usr/bin/env python3
"""
Extract frequency response data (freq, SPL, phase) from REW .mdat files.

REW .mdat files use Java serialization format ("REW Measurement Data File V2").
Each measurement contains float arrays for SPL, phase, and other data, with
an IR (impulse response) array as a landmark in the middle of each measurement's
serialized data.

Usage:
    python3 mdat2csv.py <file.mdat> [output_dir]
"""

import struct
import sys
import os
import re
import math
from pathlib import Path


def read_big_endian(data, offset, fmt):
    """Read big-endian value at offset."""
    size = struct.calcsize(fmt)
    return struct.unpack(fmt, data[offset:offset + size])[0]


def find_all(data, pattern):
    """Find all occurrences of a byte pattern in data."""
    positions = []
    start = 0
    while True:
        pos = data.find(pattern, start)
        if pos == -1:
            break
        positions.append(pos)
        start = pos + 1
    return positions


def scan_float_arrays(data):
    """
    Scan the binary data for all Java-serialized float arrays ([F).
    Returns list of (offset, length, data_start, elem_type) tuples.
    """
    arrays = []
    i = 0
    while i < len(data) - 23:
        if data[i] != 0x75:  # TC_ARRAY
            i += 1
            continue

        if data[i + 1] == 0x72:  # TC_CLASSDESC (new class definition)
            name_len = struct.unpack('>H', data[i + 2:i + 4])[0]
            if name_len > 20 or i + 4 + name_len > len(data):
                i += 1
                continue
            name = data[i + 4:i + 4 + name_len]
            if name not in (b'[F', b'[D', b'[I'):
                i += 1
                continue
            # Skip: TC_ARRAY(1) + TC_CLASSDESC(1) + name_len(2) + name + UID(8) + flags(1) + nfields(2) + TC_ENDBLOCKDATA(1) + TC_NULL(1)
            header_size = 1 + 1 + 2 + name_len + 8 + 1 + 2 + 1 + 1
            arr_len = struct.unpack('>I', data[i + header_size:i + header_size + 4])[0]
            arr_data = i + header_size + 4
            elem_type = name.decode()
            elem_size = {'[F': 4, '[D': 8, '[I': 4}[elem_type]
            arrays.append((i, arr_len, arr_data, elem_type))
            i = arr_data + arr_len * elem_size

        elif data[i + 1] == 0x71:  # TC_REFERENCE (back-reference to existing class)
            handle = struct.unpack('>I', data[i + 2:i + 6])[0]
            arr_len = struct.unpack('>I', data[i + 6:i + 10])[0]
            arr_data = i + 10
            # We need to figure out element type from handle
            # This is set per-file; we'll resolve it later
            arrays.append((i, arr_len, arr_data, f'ref:0x{handle:x}'))
            i += 10  # We'll advance past data when we resolve the type

        else:
            i += 1
            continue

    return arrays


def resolve_array_types(arrays, data):
    """Resolve reference types for arrays and compute data end positions."""
    # Find which handles map to which types
    handle_to_type = {}
    for offset, arr_len, arr_data, elem_type in arrays:
        if not elem_type.startswith('ref:'):
            # This is a new class definition; the handle is assigned sequentially
            # We don't track handles here, but we can infer from the ref arrays
            pass

    # For [F] arrays defined with TC_CLASSDESC, the subsequent TC_REFERENCE arrays
    # with the same handle point to the same type.
    # Find the first [F] definition and note what handle refs use
    float_handle = None
    int_handle = None
    double_handle = None

    for offset, arr_len, arr_data, elem_type in arrays:
        if elem_type == '[F' and float_handle is None:
            # The next ref arrays will reference this class
            # Find refs that appear after this and try to read as floats
            float_handle = 'pending'
        elif elem_type.startswith('ref:') and float_handle == 'pending':
            h = int(elem_type.split(':')[1], 16)
            float_handle = h
            break

    # Also find [I] and [D] handles
    for offset, arr_len, arr_data, elem_type in arrays:
        if elem_type == '[I':
            for o2, l2, d2, t2 in arrays:
                if t2.startswith('ref:') and o2 > offset:
                    h = int(t2.split(':')[1], 16)
                    int_handle = h
                    break
            break
    for offset, arr_len, arr_data, elem_type in arrays:
        if elem_type == '[D':
            for o2, l2, d2, t2 in arrays:
                if t2.startswith('ref:') and o2 > offset:
                    h = int(t2.split(':')[1], 16)
                    double_handle = h
                    break
            break

    # Now resolve all ref types
    resolved = []
    for offset, arr_len, arr_data, elem_type in arrays:
        if elem_type.startswith('ref:'):
            h = int(elem_type.split(':')[1], 16)
            if h == float_handle:
                resolved.append((offset, arr_len, arr_data, '[F'))
            elif h == int_handle:
                resolved.append((offset, arr_len, arr_data, '[I'))
            elif h == double_handle:
                resolved.append((offset, arr_len, arr_data, '[D'))
            else:
                resolved.append((offset, arr_len, arr_data, elem_type))
        else:
            resolved.append((offset, arr_len, arr_data, elem_type))

    return resolved


def read_float_array(data, arr_data, arr_len):
    """Read a float array from binary data."""
    return struct.unpack(f'>{arr_len}f', data[arr_data:arr_data + arr_len * 4])


def read_double_array(data, arr_data, arr_len):
    """Read a double array from binary data."""
    return struct.unpack(f'>{arr_len}d', data[arr_data:arr_data + arr_len * 8])


def parse_measdata_class(data):
    """
    Find and parse the MeasData class descriptor.
    Returns (primitive_fields, object_fields, class_desc_end) where each field
    is (type_code, name).
    """
    cd_pos = data.find(b'roomeqwizard.MeasData')
    if cd_pos == -1:
        raise ValueError("MeasData class descriptor not found")

    start = cd_pos - 3  # TC_CLASSDESC(72) + 2-byte name length
    name_len = struct.unpack('>H', data[start + 1:start + 3])[0]
    nfields = struct.unpack('>H', data[start + 3 + name_len + 9:start + 3 + name_len + 11])[0]

    pos = start + 3 + name_len + 11
    prim_fields = []
    obj_fields = []

    for _ in range(nfields):
        type_code = chr(data[pos])
        pos += 1
        fname_len = struct.unpack('>H', data[pos:pos + 2])[0]
        pos += 2
        fname = data[pos:pos + fname_len].decode()
        pos += fname_len

        if type_code in ('L', '['):
            if data[pos] == 0x74:  # TC_STRING
                cname_len = struct.unpack('>H', data[pos + 1:pos + 3])[0]
                pos += 3 + cname_len
            elif data[pos] == 0x71:  # TC_REFERENCE
                pos += 5
            else:
                pos += 1

        if type_code in ('D', 'I', 'F', 'Z', 'J', 'S', 'B', 'C'):
            prim_fields.append((type_code, fname))
        else:
            obj_fields.append((type_code, fname))

    # Skip TC_ENDBLOCKDATA(78) + TC_NULL(70) for super class
    pos += 2
    return prim_fields, obj_fields, pos


TYPE_SIZES = {'D': 8, 'I': 4, 'F': 4, 'Z': 1, 'J': 8, 'S': 2, 'B': 1, 'C': 2}
TYPE_FMTS = {'D': '>d', 'I': '>i', 'F': '>f', 'Z': '>?', 'J': '>q', 'S': '>h', 'B': '>b', 'C': '>H'}


def read_primitive_fields(data, offset, prim_fields):
    """Read all primitive fields starting at offset. Returns dict of field values."""
    values = {}
    pos = offset
    for type_code, name in prim_fields:
        sz = TYPE_SIZES[type_code]
        values[name] = struct.unpack(TYPE_FMTS[type_code], data[pos:pos + sz])[0]
        pos += sz
    return values, pos


def compute_frequencies(params):
    """Compute frequency axis from measurement parameters."""
    n = params['dataLength']
    start = params['startFreq']
    if params['isLogSpaced']:
        step = params['logStep']
        return [start * (step ** i) for i in range(n)]
    else:
        step = params['freqStep']
        return [start + step * i for i in range(n)]


def find_measurement_primitives(data, class_desc_end, prim_fields):
    """
    Find all MeasData instances' primitive field blocks.
    The first instance starts right at class_desc_end.
    Subsequent instances start after TC_OBJECT(73) + TC_REFERENCE(71) + handle(4 bytes).
    """
    prim_size = sum(TYPE_SIZES[tc] for tc, _ in prim_fields)

    # First instance
    instances = [class_desc_end]

    # For subsequent instances, search for a pattern:
    # We look for the dataLength field (int = specific value) preceded by
    # the right amount of primitive data.
    # Read first instance to get expected values
    first_params, _ = read_primitive_fields(data, class_desc_end, prim_fields)
    data_length = first_params['dataLength']
    sample_rate = first_params['sampleRate']

    # Calculate the offset of dataLength within primitive fields
    dl_offset = 0
    for tc, name in prim_fields:
        if name == 'dataLength':
            break
        dl_offset += TYPE_SIZES[tc]

    # Search for subsequent instances:
    # Pattern: 73 71 <4-byte handle> then primitive data starting with
    # known values (first few doubles likely 0.0)
    # The dataLength field at dl_offset should match
    search_start = class_desc_end + prim_size
    while search_start < len(data) - prim_size - 6:
        # Look for TC_OBJECT(73) + TC_REFERENCE(71) + 4-byte handle
        pos = data.find(b'\x73\x71', search_start)
        if pos == -1:
            break
        # Check if the data at pos+6 (after 73 71 xx xx xx xx) looks like
        # primitive fields with a matching dataLength
        prim_start = pos + 6
        if prim_start + prim_size > len(data):
            break
        candidate_dl = struct.unpack('>i', data[prim_start + dl_offset:prim_start + dl_offset + 4])[0]
        if candidate_dl == data_length:
            # Additional validation: check sampleRate
            sr_offset = 0
            for tc, name in prim_fields:
                if name == 'sampleRate':
                    break
                sr_offset += TYPE_SIZES[tc]
            candidate_sr = struct.unpack('>i', data[prim_start + sr_offset:prim_start + sr_offset + 4])[0]
            if candidate_sr == sample_rate:
                instances.append(prim_start)
        search_start = pos + 2

    return instances


def find_html_descriptions(data):
    """Find HTML thumbnail descriptions for measurement metadata."""
    descriptions = []
    for m in re.finditer(b'<BODY>(.*?)</HTML>', data, re.DOTALL):
        content = m.group(1).decode('utf-8', errors='replace')
        parts = content.split('<BR>')
        desc = {
            'raw': content,
            'date': parts[0] if len(parts) > 0 else '',
            'time': parts[1] if len(parts) > 1 else '',
            'freq_range': parts[2] if len(parts) > 2 else '',
            'spl_range': parts[3] if len(parts) > 3 else '',
        }
        descriptions.append(desc)
    return descriptions


def find_measurement_names(data, ir_positions):
    """Try to find measurement names from shortDesc or notes fields."""
    names = []
    # Look for strings after each IR data block
    for ir_idx, ir_pos in enumerate(ir_positions):
        ir_end = ir_pos + 10 + 131072 * 4
        # Search for TC_STRING patterns in the gap after IR
        search_area = data[ir_end:ir_end + 5000]
        found_name = None
        pos = 0
        while pos < len(search_area) - 3:
            if search_area[pos] == 0x74:  # TC_STRING
                slen = struct.unpack('>H', search_area[pos + 1:pos + 3])[0]
                if 3 <= slen <= 500 and pos + 3 + slen <= len(search_area):
                    try:
                        s = search_area[pos + 3:pos + 3 + slen].decode('utf-8')
                        # Skip Java class/type strings
                        if not any(x in s for x in ['java', 'javax', 'roomeq', 'swing', 'awt',
                                                      'HERMITE', 'TUKEY', 'HANN', 'PERCENT',
                                                      'Ljava', '64-bit', '32-bit']):
                            if not s.startswith(('[', 'L[')):
                                if found_name is None or len(s) > len(found_name):
                                    found_name = s
                    except UnicodeDecodeError:
                        pass
            pos += 1
        names.append(found_name)
    return names


def identify_phase_array(arrays, data, data_length):
    """
    Identify the wrapped phase array: min near -180, max near +180, correct length.
    """
    for offset, arr_len, arr_data, elem_type in arrays:
        if elem_type != '[F' or arr_len != data_length:
            continue
        vals = read_float_array(data, arr_data, arr_len)
        vmin, vmax = min(vals), max(vals)
        if vmin < -150 and vmax > 150 and vmax < 200 and vmin > -200:
            return vals
    return None


def identify_spl_array(arrays, data, data_length):
    """
    Identify SPL arrays among the candidates.
    Returns (rawValues, splValues) - splValues may be None if only one SPL array found.
    """
    spl_candidates = []
    for offset, arr_len, arr_data, elem_type in arrays:
        if elem_type != '[F' or arr_len != data_length:
            continue
        vals = read_float_array(data, arr_data, arr_len)
        vmin, vmax = min(vals), max(vals)
        vmean = sum(vals) / len(vals)
        # SPL arrays: mean typically 40-110 dB, all values positive, significant variation
        if 30 < vmean < 120 and vmin > -10:
            variance = sum((v - vmean) ** 2 for v in vals) / len(vals)
            std = variance ** 0.5
            if std > 1.5:
                spl_candidates.append((offset, vals, vmean, std, vmin, vmax))

    if not spl_candidates:
        return None, None

    # Sort by offset (field order: rawValues at field 85, splValues at field 109)
    spl_candidates.sort(key=lambda x: x[0])

    if len(spl_candidates) == 1:
        return spl_candidates[0][1], None

    # rawValues is first, splValues is second (if present and distinct)
    raw = spl_candidates[0][1]
    # splValues should be similar to rawValues but possibly offset by calibration
    # It's the second SPL-range array in field order
    spl = spl_candidates[1][1] if len(spl_candidates) > 1 else None
    return raw, spl


def parse_mdat(filepath):
    """
    Parse an REW .mdat file and extract measurements.
    Returns list of measurement dicts with 'freq', 'spl', 'phase', 'name', 'params'.
    """
    with open(filepath, 'rb') as f:
        data = f.read()

    # Verify header
    header_str = data.find(b'REW Measurement Data File')
    if header_str == -1:
        raise ValueError("Not a valid REW .mdat file")

    # Parse MeasData class descriptor
    prim_fields, obj_fields, class_desc_end = parse_measdata_class(data)

    # Find all MeasData instances' primitive fields
    prim_starts = find_measurement_primitives(data, class_desc_end, prim_fields)

    # Parse primitive fields for each measurement
    all_params = []
    for ps in prim_starts:
        params, _ = read_primitive_fields(data, ps, prim_fields)
        all_params.append(params)

    num_measurements = len(all_params)

    # Scan all arrays in the file
    raw_arrays = scan_float_arrays(data)
    arrays = resolve_array_types(raw_arrays, data)

    # Find IR arrays: power-of-2 sized float arrays that appear once per measurement.
    # These serve as landmarks separating pre-IR (distortion etc.) from post-IR (phase, SPL).
    ir_candidates = {}
    for o, l, d, t in arrays:
        if t == '[F' and l >= 512 and l & (l - 1) == 0:  # power of 2, >= 512
            ir_candidates.setdefault(l, []).append(o)
    # The IR length that matches measurement count is the one we want
    ir_length = None
    ir_offsets = []
    for length, offsets in sorted(ir_candidates.items(), reverse=True):
        if len(offsets) == num_measurements:
            ir_length = length
            ir_offsets = sorted(offsets)
            break

    # Get HTML descriptions for naming
    html_descs = find_html_descriptions(data)

    prim_size = sum(TYPE_SIZES[tc] for tc, _ in prim_fields)
    measurements = []
    for meas_idx in range(num_measurements):
        params = all_params[meas_idx]
        data_length = params['dataLength']
        freqs = compute_frequencies(params)

        meas_start = prim_starts[meas_idx]
        if meas_idx + 1 < len(prim_starts):
            meas_end = prim_starts[meas_idx + 1]
        else:
            meas_end = len(data)
        prim_end = meas_start + prim_size

        # If we have IR landmarks, prefer post-IR arrays (where phase/SPL live)
        if ir_offsets and meas_idx < len(ir_offsets):
            ir_offset = ir_offsets[meas_idx]
            ir_data_end = ir_offset + 10 + ir_length * 4  # 10 = array header
            # Post-IR: between IR end and next measurement's primitive start
            post_ir = [(o, l, d, t) for o, l, d, t in arrays
                       if o > ir_data_end and o < meas_end]
            phase = identify_phase_array(post_ir, data, data_length)
            raw_spl, cal_spl = identify_spl_array(post_ir, data, data_length)
        else:
            phase = None
            raw_spl = None
            cal_spl = None

        # Fallback: search all arrays within this measurement's range
        if phase is None or raw_spl is None:
            all_meas = [(o, l, d, t) for o, l, d, t in arrays
                        if o > prim_end and o < meas_end]
            if phase is None:
                phase = identify_phase_array(all_meas, data, data_length)
            if raw_spl is None:
                raw_spl, cal_spl = identify_spl_array(all_meas, data, data_length)

        # Use calibrated SPL if available, otherwise raw
        spl = cal_spl if cal_spl is not None else raw_spl

        # Build measurement name
        if meas_idx < len(html_descs):
            desc = html_descs[meas_idx]
            name = f"measurement_{meas_idx + 1}_{desc['date']}_{desc['time']}"
        else:
            name = f"measurement_{meas_idx + 1}"
        # Clean name for filename
        name = re.sub(r'[^\w\-.]', '_', name)
        name = re.sub(r'_+', '_', name).strip('_')

        measurements.append({
            'name': name,
            'freq': freqs,
            'spl': spl,
            'phase': phase,
            'params': params,
            'html': html_descs[meas_idx] if meas_idx < len(html_descs) else None,
        })

    return measurements


def sanitize_filename(name, max_len=100):
    """Make a string safe for use as a filename."""
    name = re.sub(r'[^\w\-. ]', '_', name)
    name = re.sub(r'_+', '_', name).strip('_')
    if len(name) > max_len:
        name = name[:max_len]
    return name


def export_csv(measurement, output_dir):
    """Export a single measurement to CSV."""
    os.makedirs(output_dir, exist_ok=True)
    name = sanitize_filename(measurement['name'])
    filepath = os.path.join(output_dir, f"{name}.csv")

    freqs = measurement['freq']
    spl = measurement['spl']
    phase = measurement['phase']

    with open(filepath, 'w') as f:
        f.write("freq_hz,spl_db,phase_deg\n")
        for i in range(len(freqs)):
            freq = freqs[i]
            s = f"{spl[i]:.6f}" if spl is not None else ""
            p = f"{phase[i]:.6f}" if phase is not None else ""
            f.write(f"{freq:.6f},{s},{p}\n")

    return filepath


def export_recordings_json(measurements, csv_paths, output_dir):
    """
    Export a recordings.json file conforming to the roomeq input_schema.json.
    Each measurement becomes a speaker entry referencing its CSV file.
    """
    import json

    speakers = {}
    for m, csv_path in zip(measurements, csv_paths):
        if csv_path is None:
            continue
        # Use relative path from recordings.json location
        rel_path = os.path.relpath(csv_path, output_dir)
        key = sanitize_filename(m['name'])
        speakers[key] = {
            "path": rel_path,
            "name": m['name'],
        }

    config = {
        "version": "1.3.0",
        "speakers": speakers,
    }

    json_path = os.path.join(output_dir, "recordings.json")
    with open(json_path, 'w') as f:
        json.dump(config, f, indent=2)
        f.write('\n')

    return json_path


def main():
    if len(sys.argv) < 2:
        print(f"Usage: {sys.argv[0]} <file.mdat> [output_dir]")
        sys.exit(1)

    mdat_path = sys.argv[1]
    if len(sys.argv) > 2:
        output_dir = sys.argv[2]
    else:
        output_dir = os.path.splitext(mdat_path)[0] + "_csv"

    print(f"Parsing: {mdat_path}")
    measurements = parse_mdat(mdat_path)

    print(f"Found {len(measurements)} measurements")
    csv_paths = []
    for i, m in enumerate(measurements):
        params = m['params']
        has_spl = m['spl'] is not None
        has_phase = m['phase'] is not None
        html = m['html']
        spl_range = ""
        if has_spl:
            spl_range = f" SPL=[{min(m['spl']):.1f}..{max(m['spl']):.1f}]"
        phase_range = ""
        if has_phase:
            phase_range = f" Phase=[{min(m['phase']):.1f}..{max(m['phase']):.1f}]"
        html_info = ""
        if html:
            html_info = f" ({html['spl_range'].strip()})"

        print(f"  [{i + 1}] {params['dataLength']} pts, "
              f"{params['startFreq']:.1f}-{params['endFreq']:.0f} Hz, "
              f"{'SPL:yes' if has_spl else 'SPL:NO'}{spl_range}{html_info}, "
              f"{'Phase:yes' if has_phase else 'Phase:NO'}{phase_range}")

        if has_spl or has_phase:
            filepath = export_csv(m, output_dir)
            csv_paths.append(filepath)
            print(f"      -> {filepath}")
        else:
            csv_paths.append(None)
            print(f"      -> SKIPPED (no SPL or phase data found)")

    json_path = export_recordings_json(measurements, csv_paths, output_dir)
    print(f"\n  recordings.json -> {json_path}")


if __name__ == '__main__':
    main()
