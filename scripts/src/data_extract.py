"""Functions for extracting data from roomeq output JSON."""

import math


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
