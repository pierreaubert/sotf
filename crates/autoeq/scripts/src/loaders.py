"""File I/O functions for loading roomeq data files."""

import json
import sys
from pathlib import Path


def load_roomeq_json(filepath: Path) -> dict:
    """Load and parse roomeq JSON output file."""
    if not filepath.exists():
        print(f"Error: File not found: {filepath}")
        sys.exit(1)
    with open(filepath, "r") as f:
        return json.load(f)
