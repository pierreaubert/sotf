# Track: RoomEQ System Configuration Refactor

## 1. Overview
The current RoomEQ configuration mixes measurement sources with logical channel definitions. The goal is to introduce a `system` section that explicitly maps logical roles (e.g., "L", "R", "LFE") to measurement keys and defines system-wide topology (Stereo vs Home Cinema) and subwoofer handling strategies. This is intended to fix issues with 2.1 system configuration.

## 2. Functional Requirements

### 2.1 Schema Updates
*   **New `system` Section:** Add a `SystemConfig` struct to `RoomConfig`.
    *   **`model`:** Enum (`Stereo`, `HomeCinema`, `Custom`).
    *   **`speakers`:** Map of Logical Role (String) -> Measurement Key (String).
        *   Example: `"L": "left_measurement"`
    *   **`subwoofers`:** Configuration for subwoofer handling.
        *   **`config`:** Enum (`Single`, `Mso`, `Dba`).
        *   **Mapping:** Map of Subwoofer Measurement Key -> Main Speaker Logical Role (for pairing/alignment).
            *   Example: `"sub0": "L"` (Sub 0 is paired with Left main).

### 2.2 Logic Updates
*   **`optimize_room`:**
    *   Use the `system.speakers` map to identify which measurements correspond to which logical channels.
    *   Use `system.subwoofers` to determine how to process bass management and alignment.
    *   Ensure backward compatibility or clear migration for existing configs (optional, but good practice).

## 3. Example JSON
```json
"system": {
    "model": "stereo",
    "speakers": {
        "L": "left",
        "R": "right",
        "LFE": "sub0"
    },
    "subwoofers": {
        "config": "single",
        "sub0": "L"
    }
}
```

## 4. Acceptance Criteria
*   `RoomConfig` deserializes the new `system` section correctly.
*   `optimize_room` correctly identifies "L" and "R" and "LFE" based on the mapping.
*   Subwoofer processing respects the `config` strategy and pairing.
