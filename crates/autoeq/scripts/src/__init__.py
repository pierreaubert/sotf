"""RoomEQ display utilities."""

# Smoothing options in octave fractions
# Index 0 is the default — 1/6 octave preserves narrow Q filters while
# still reducing measurement noise.
SMOOTHING_OPTIONS = [
    ("1/6 oct", 1/6),
    ("1/3 oct", 1/3),
    ("1/2 oct", 1/2),
    ("1/1 oct", 1.0),
    ("1/12 oct", 1/12),
    ("1/24 oct", 1/24),
    ("1/48 oct", 1/48),
    ("Raw", None),
]

DEFAULT_SMOOTHING = 1/6  # 1/6 octave
