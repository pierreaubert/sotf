"""RoomEQ display utilities."""

# Smoothing options in octave fractions
# Index 0 is the default
SMOOTHING_OPTIONS = [
    ("1/1 oct", 1.0),
    ("1/3 oct", 1/3),
    ("1/6 oct", 1/6),
    ("1/12 oct", 1/12),
    ("1/24 oct", 1/24),
    ("Raw", None),
]

DEFAULT_SMOOTHING = 1.0  # 1/1 octave
