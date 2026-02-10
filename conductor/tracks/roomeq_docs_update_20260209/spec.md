# Track: RoomEQ Docs & Schema Update

## 1. Overview
Following the implementation of `SystemConfig` in the `roomeq` crate, the JSON schema and associated documentation must be updated to match. This ensures users can validate their v2.1 configuration files.

## 2. Changes

### 2.1 Input Schema (`input_schema.json`)
*   Add `system` property to root object.
*   Define `SystemConfig` schema:
    *   `model`: Enum ["stereo", "home_cinema", "custom"]
    *   `speakers`: Map<String, String> (Role -> Measurement Key)
    *   `subwoofers`: Object
        *   `config`: Enum ["single", "mso", "dba"]
        *   `mapping`: Map<String, String> (Sub Key -> Main Role) - flattened in Rust but explicit in JSON?
        *   Wait, in Rust `SubwooferSystemConfig` has `#[serde(flatten)] pub mapping: HashMap<String, String>`.
        *   This means the JSON structure is:
            ```json
            "subwoofers": {
                "config": "single",
                "sub_meas_key": "main_role",
                "another_sub": "another_role"
            }
            ```
        *   Schema for flattened map is tricky. `additionalProperties` with string values can handle it, excluding "config".

### 2.2 Input Format Doc (`INPUT_FORMAT.md`)
*   Add "System Configuration" section describing `system`, `model`, `speakers`, `subwoofers`.
*   Add example for 2.1 system.

### 2.3 README (`crates/autoeq/bin/roomeq/README.md`)
*   Ensure it links to updated format docs.

### 2.4 Convert Recording (`convert_recording.rs`)
*   Review if any updates are needed beyond the `system: None` initialization already applied.

## 3. Acceptance Criteria
*   `input_schema.json` validates a correct v2.1 config.
*   `INPUT_FORMAT.md` accurately describes the new section.
