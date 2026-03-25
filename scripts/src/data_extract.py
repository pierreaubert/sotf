"""Functions for extracting data from roomeq output JSON."""

import math


# Channel order mapping for sorting (classical order)
CHANNEL_ORDER_MAP = {
    "L": 10, "LEFT": 10,
    "R": 20, "RIGHT": 20,
    "C": 30, "CENTER": 30,
    "LFE": 40, "SUB": 40, "SUBWOOFER": 40, "LFE1": 41, "LFE2": 42,
    "SL": 50, "SURROUND LEFT": 50, "LS": 50,
    "SR": 60, "SURROUND RIGHT": 60, "RS": 60,
    "SBL": 70, "SURROUND BACK LEFT": 70, "LBS": 70, "LB": 70,
    "SBR": 80, "SURROUND BACK RIGHT": 80, "RBS": 80, "RB": 80,
    "FHL": 90, "FRONT HEIGHT LEFT": 90,
    "FHR": 100, "FRONT HEIGHT RIGHT": 100,
    "BHL": 110, "BACK HEIGHT LEFT": 110,
    "BHR": 120, "BACK HEIGHT RIGHT": 120,
}


def get_channel_sort_key(channel_name: str) -> tuple[int, str]:
    """Get sort key for a channel name."""
    name_upper = channel_name.upper()
    # Try exact match
    if name_upper in CHANNEL_ORDER_MAP:
        return (CHANNEL_ORDER_MAP[name_upper], channel_name)

    # Try to see if it starts with one of the keys (e.g. "L (tweeter)")
    for key, order in CHANNEL_ORDER_MAP.items():
        if name_upper.startswith(key) and (len(name_upper) == len(key) or not name_upper[len(key)].isalnum()):
            return (order, channel_name)

    # Default: large number to put unknown at the end
    return (1000, channel_name)


def compute_y_range(curves: list[dict | None]) -> tuple[float, float]:
    """Compute y-axis range from curve data: 50 dB span, max rounded up to next multiple of 5."""
    all_spl = []
    for curve in curves:
        if curve and "spl" in curve:
            all_spl.extend(curve["spl"])

    if not all_spl:
        return (-20, 30)

    upper = math.ceil(max(all_spl) / 5) * 5
    return (upper - 50, upper)


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


def extract_eq_passes(channel_data: dict) -> list[dict]:
    """
    Extract EQ plugins from a channel, grouped by pass label.

    The 3-pass pipeline labels EQ plugins as:
    - "cea2034_speaker_correction" (Pass 1)
    - "room_eq_correction" (Pass 2, or unlabeled)
    - "user_preference" (Pass 3)

    Unlabeled EQ plugins are assigned to "Room EQ".

    Returns:
        List of dicts with keys: label, display_name, filters, color
    """
    plugins = channel_data.get("plugins", [])

    PASS_DISPLAY_NAMES = {
        "cea2034_speaker_correction": "Pass 1: Speaker Correction (CEA2034)",
        "room_eq_correction": "Pass 2: Room EQ",
        "user_preference": "Pass 3: User Preference",
    }

    PASS_COLORS = {
        "cea2034_speaker_correction": "rgba(255, 165, 0, 0.9)",   # orange
        "room_eq_correction": "rgba(100, 100, 255, 0.9)",         # blue
        "user_preference": "rgba(180, 100, 255, 0.9)",            # purple
    }

    passes: list[dict] = []

    for plugin in plugins:
        if plugin.get("plugin_type") != "eq":
            continue

        params = plugin.get("parameters", {})
        label = params.get("label", "")
        filters = params.get("filters", [])

        if not filters:
            continue

        display_name = PASS_DISPLAY_NAMES.get(label, "Room EQ")
        color = PASS_COLORS.get(label, "rgba(100, 100, 255, 0.9)")

        passes.append({
            "label": label,
            "display_name": display_name,
            "filters": filters,
            "color": color,
        })

    return passes


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
