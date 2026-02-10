"""Input JSON parsing for extracting measurement file paths."""

from pathlib import Path


def extract_measurement_paths(input_data: dict, json_dir: Path) -> dict[str, Path]:
    """
    Extract original measurement file paths from input JSON.

    Handles all SpeakerConfig variants from the input schema:
    - String path: "measurements/left.csv"
    - Object with path: {"path": "measurements/left.csv"}
    - Array of measurements (averaged): ["pos1.csv", "pos2.csv"]
    - SpeakerGroup: {"name": "...", "measurements": [...]}
    - MultiSubGroup: {"name": "...", "subwoofers": [...]}
    - DBAConfig: {"name": "...", "front": [...], "rear": [...]}

    Args:
        input_data: The roomeq input JSON data
        json_dir: Directory containing the JSON file (for resolving relative paths)

    Returns:
        Dict mapping speaker key to resolved measurement Path (CSV or WAV).
        Keys are the raw speaker keys from input_data["speakers"].
    """
    paths = {}
    speakers = input_data.get("speakers", {})

    for channel_name, speaker_data in speakers.items():
        measurement_path = _extract_path_from_speaker(speaker_data, json_dir)
        if measurement_path:
            paths[channel_name] = measurement_path

    return paths


def extract_channel_measurement_paths(input_data: dict, json_dir: Path) -> dict[str, Path]:
    """
    Extract measurement file paths mapped by output channel names.

    When a system config with speaker role mapping exists, uses the
    system.speakers mapping to translate from output channel names (e.g. "L", "R")
    to measurement keys (e.g. "left0", "right0") and then to file paths.

    Without a system config, falls back to using the raw speaker keys as channel names.

    Args:
        input_data: The roomeq input JSON data
        json_dir: Directory containing the JSON file (for resolving relative paths)

    Returns:
        Dict mapping output channel name to resolved measurement Path.
    """
    raw_paths = extract_measurement_paths(input_data, json_dir)

    system = input_data.get("system")
    if not system:
        return raw_paths

    # Build mapping: output channel name -> measurement key
    system_speakers = system.get("speakers", {})
    # Also include subwoofer mappings (sub measurement key -> role)
    subwoofers = system.get("subwoofers", {})

    channel_paths = {}

    # Map logical roles (L, R, C, LFE, etc.) to measurement keys
    for role, measurement_key in system_speakers.items():
        if measurement_key in raw_paths:
            channel_paths[role] = raw_paths[measurement_key]

    # Map subwoofer measurement keys directly (they appear as channel names too)
    for key, value in subwoofers.items():
        if key in ("config", "crossover"):
            continue
        # key is a sub measurement key (e.g. "sub0"), value is a role (e.g. "L")
        if key in raw_paths and key not in channel_paths:
            channel_paths[key] = raw_paths[key]

    # Fallback: include raw measurement paths for any keys not already covered.
    # This ensures paths are available even if system.speakers has stale values.
    for key, path in raw_paths.items():
        if key not in channel_paths:
            channel_paths[key] = path

    return channel_paths


def _resolve_path(path_str: str, json_dir: Path) -> Path:
    """Resolve a path string relative to the JSON directory."""
    p = Path(path_str)
    if not p.is_absolute():
        p = json_dir / p
    return p


def _extract_path_from_speaker(speaker_data, json_dir: Path) -> Path | None:
    """Extract measurement file path from a speaker configuration."""
    if speaker_data is None:
        return None

    # Simple string path: "measurements/left.csv"
    if isinstance(speaker_data, str):
        return _resolve_path(speaker_data, json_dir)

    # Array of MeasurementRefs (multiple measurements for averaging) - take first
    if isinstance(speaker_data, list):
        for item in speaker_data:
            path = _extract_path_from_measurement_ref(item, json_dir)
            if path:
                return path
        return None

    # Dict - could be:
    #   {"path": "..."} (MeasurementRef object)
    #   {"name": "...", "measurements": [...]} (SpeakerGroup)
    #   {"name": "...", "subwoofers": [...]} (MultiSubGroup)
    #   {"name": "...", "front": [...], "rear": [...]} (DBAConfig)
    if isinstance(speaker_data, dict):
        # SpeakerGroup: multi-driver with crossover
        if "measurements" in speaker_data:
            for measurement in speaker_data["measurements"]:
                path = _extract_path_from_measurement_source(measurement, json_dir)
                if path:
                    return path
            return None

        # MultiSubGroup: multiple subwoofers
        if "subwoofers" in speaker_data:
            for sub in speaker_data["subwoofers"]:
                path = _extract_path_from_measurement_source(sub, json_dir)
                if path:
                    return path
            return None

        # DBAConfig: front/rear arrays
        if "front" in speaker_data:
            for item in speaker_data["front"]:
                path = _extract_path_from_measurement_source(item, json_dir)
                if path:
                    return path
            return None

        # MeasurementRef object: {"path": "...", "name": "..."}
        if "path" in speaker_data:
            return _resolve_path(speaker_data["path"], json_dir)

    return None


def _extract_path_from_measurement_source(source, json_dir: Path) -> Path | None:
    """Extract path from a MeasurementSource (single ref or array of refs)."""
    if source is None:
        return None

    # Single MeasurementRef
    path = _extract_path_from_measurement_ref(source, json_dir)
    if path:
        return path

    # Array of MeasurementRefs (multiple for averaging) - take first
    if isinstance(source, list):
        for item in source:
            path = _extract_path_from_measurement_ref(item, json_dir)
            if path:
                return path

    return None


def _extract_path_from_measurement_ref(ref, json_dir: Path) -> Path | None:
    """Extract path from a MeasurementRef (string or object with path)."""
    if ref is None:
        return None

    # String path: "measurements/left.csv"
    if isinstance(ref, str):
        return _resolve_path(ref, json_dir)

    # Object: {"path": "measurements/left.csv", "name": "..."}
    if isinstance(ref, dict):
        path_str = ref.get("path")
        if path_str:
            return _resolve_path(path_str, json_dir)

    return None
