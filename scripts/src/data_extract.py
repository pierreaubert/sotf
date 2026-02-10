"""Functions for extracting data from roomeq output JSON."""

from pathlib import Path


def compute_y_range(curves: list[dict | None], padding: float = 2.0) -> tuple[float, float]:
    """Compute dynamic y-axis range from curve data."""
    all_spl = []
    for curve in curves:
        if curve and "spl" in curve:
            all_spl.extend(curve["spl"])

    if not all_spl:
        return (-20, 20)

    min_spl = min(all_spl)
    max_spl = max(all_spl)
    return (min_spl - padding, max_spl + padding)


def compute_average_spl_in_range(
    curve: dict, min_freq: float = 20.0, max_freq: float = 1200.0
) -> float:
    """Compute average SPL in a frequency range."""
    if not curve or "freq" not in curve or "spl" not in curve:
        return 0.0

    freq = curve["freq"]
    spl = curve["spl"]

    values_in_range = [
        s for f, s in zip(freq, spl) if min_freq <= f <= max_freq
    ]

    if not values_in_range:
        return 0.0

    return sum(values_in_range) / len(values_in_range)


def extract_ir_wav_paths(channel_data: dict) -> list[tuple[str, str]]:
    """
    Extract impulse response WAV file paths from a channel's plugin configuration.

    Looks for:
    - "convolution" plugins with "ir_file" parameter

    Returns:
        List of (name, path) tuples
    """
    ir_paths = []

    # Check main plugins for convolution
    plugins = channel_data.get("plugins", [])
    for plugin in plugins:
        if plugin.get("plugin_type") == "convolution":
            ir_file = plugin.get("parameters", {}).get("ir_file")
            if ir_file:
                ir_paths.append(("main", ir_file))

    # Check driver chains for convolution plugins
    drivers = channel_data.get("drivers", [])
    for driver in drivers:
        driver_name = driver.get("name", "driver")
        driver_plugins = driver.get("plugins", [])
        for plugin in driver_plugins:
            if plugin.get("plugin_type") == "convolution":
                ir_file = plugin.get("parameters", {}).get("ir_file")
                if ir_file:
                    ir_paths.append((driver_name, ir_file))

    return ir_paths


def get_all_ir_wav_paths(data: dict, json_dir: Path) -> dict[str, list[tuple[str, Path]]]:
    """
    Extract all impulse response WAV file paths from all channels.

    Args:
        data: The roomeq JSON data
        json_dir: Directory containing the JSON file (for resolving relative paths)

    Returns:
        Dict mapping channel name to list of (ir_name, resolved_path) tuples
    """
    all_paths = {}
    channels = data.get("channels", {})

    for channel_name, channel_data in channels.items():
        ir_paths = extract_ir_wav_paths(channel_data)
        if ir_paths:
            resolved_paths = []
            for ir_name, ir_file in ir_paths:
                ir_path = Path(ir_file)
                # Resolve relative paths against JSON file directory
                if not ir_path.is_absolute():
                    ir_path = json_dir / ir_path
                resolved_paths.append((ir_name, ir_path))
            all_paths[channel_name] = resolved_paths

    return all_paths


def extract_crossover_frequencies(channel_data: dict) -> list[float]:
    """
    Extract crossover frequencies from a channel's plugin configuration.

    Looks for:
    - "crossover" plugins in driver chains (active crossovers)
    - "band_split" plugins in main chain (mixed mode crossovers)

    Returns:
        Sorted list of unique crossover frequencies in Hz
    """
    crossover_freqs = set()

    # Check main plugins for band_split and crossover
    plugins = channel_data.get("plugins", [])
    for plugin in plugins:
        if plugin.get("plugin_type") == "band_split":
            freq = plugin.get("parameters", {}).get("frequency")
            if freq:
                crossover_freqs.add(float(freq))
        elif plugin.get("plugin_type") == "crossover":
            freq = plugin.get("parameters", {}).get("frequency")
            if freq:
                crossover_freqs.add(float(freq))

    # Check driver chains for crossover plugins
    drivers = channel_data.get("drivers", [])
    for driver in drivers:
        driver_plugins = driver.get("plugins", [])
        for plugin in driver_plugins:
            if plugin.get("plugin_type") == "crossover":
                freq = plugin.get("parameters", {}).get("frequency")
                if freq:
                    crossover_freqs.add(float(freq))

    return sorted(crossover_freqs)


def get_driver_initial_curves(channel_data: dict) -> list[tuple[str, dict]] | None:
    """Extract per-driver initial curves from a channel's driver chains.

    Returns:
        List of (driver_name, curve_data) tuples, or None if no per-driver curves exist.
        Each curve_data has "freq" and "spl" keys.
    """
    drivers = channel_data.get("drivers", [])
    if not drivers:
        return None

    result = []
    for driver in drivers:
        initial_curve = driver.get("initial_curve")
        if initial_curve and "freq" in initial_curve and "spl" in initial_curve:
            name = driver.get("name", f"driver_{driver.get('index', '?')}")
            result.append((name, initial_curve))

    return result if result else None


def get_summing_groups(input_data: dict | None, channel_names: list[str]) -> list[tuple[str, list[str]]]:
    """Determine which channels should be summed together for listening position curves.

    For a 2.1 system (config: "single"), the sub serves all mains:
      - "L + LFE" → ["L", "LFE"]
      - "R + LFE" → ["R", "LFE"]

    Args:
        input_data: Input JSON data containing system topology
        channel_names: Available channel names from the output JSON

    Returns:
        List of (label, [channel_name, ...]) tuples.
        Empty list if no meaningful summing groups can be determined.
    """
    if not input_data:
        return []

    system = input_data.get("system", {})
    subs_config = system.get("subwoofers")
    if not subs_config:
        return []

    speakers = system.get("speakers", {})
    available = set(channel_names)
    strategy = subs_config.get("config", "single")

    # Find subwoofer logical roles from the mapping
    # The mapping has measurement_key → alignment_role (e.g., "lfe" → "L")
    reserved_keys = {"config", "crossover"}
    sub_roles: list[str] = []
    sub_alignment: dict[str, str] = {}  # sub_role → alignment main role
    for key, main_role in subs_config.items():
        if key in reserved_keys:
            continue
        # Find the logical role for this sub measurement key
        for role, meas_key in speakers.items():
            if meas_key == key and role in available:
                sub_roles.append(role)
                sub_alignment[role] = main_role
                break

    if not sub_roles:
        return []

    # Identify main (non-sub) channels
    main_roles = [ch for ch in channel_names if ch not in sub_roles]

    groups: list[tuple[str, list[str]]] = []

    if strategy == "single":
        # Single sub serves all main channels
        sub_role = sub_roles[0]
        for main_role in main_roles:
            if main_role in available:
                groups.append((f"{main_role} + {sub_role}", [main_role, sub_role]))
    else:
        # MSO / DBA: each sub is paired with its alignment main
        for sub_role, main_role in sub_alignment.items():
            if main_role in available:
                groups.append((f"{main_role} + {sub_role}", [main_role, sub_role]))

    return groups


def get_all_crossover_frequencies(data: dict) -> list[float]:
    """
    Extract all unique crossover frequencies from all channels.

    Returns:
        Sorted list of unique crossover frequencies in Hz
    """
    all_freqs = set()
    channels = data.get("channels", {})

    for channel_data in channels.values():
        freqs = extract_crossover_frequencies(channel_data)
        all_freqs.update(freqs)

    return sorted(all_freqs)
