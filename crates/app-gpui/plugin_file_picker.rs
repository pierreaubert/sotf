#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilePickerOpenTarget {
    Sofa,
    Ir,
    AbConfig(&'static str),
}

pub fn file_picker_open_target(engine_key: &str) -> Option<FilePickerOpenTarget> {
    match engine_key {
        "sofa_file" => Some(FilePickerOpenTarget::Sofa),
        "ir_file" | "room_ir_file" => Some(FilePickerOpenTarget::Ir),
        "path_a_config" => Some(FilePickerOpenTarget::AbConfig("a")),
        "path_b_config" => Some(FilePickerOpenTarget::AbConfig("b")),
        _ => None,
    }
}

/// ui.md Phase 3 pilot: the convolution resource row shows the IR file's base
/// name so long paths cannot widen the editor. The full path remains
/// available on demand through the row tooltip.
pub fn convolution_ir_display_name(ir_file: &str) -> &str {
    ir_file
        .rsplit(['/', '\\'])
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or(ir_file)
}

#[cfg(test)]
mod tests {
    use super::convolution_ir_display_name;

    #[test]
    fn resource_row_shows_base_name_for_long_paths() {
        assert_eq!(
            convolution_ir_display_name("/very/long/path/to/irs/large-hall-v2.wav"),
            "large-hall-v2.wav"
        );
        assert_eq!(
            convolution_ir_display_name("C:\\Impulse Responses\\plate stereo.wav"),
            "plate stereo.wav"
        );
        assert_eq!(convolution_ir_display_name("room.wav"), "room.wav");
    }

    #[test]
    fn empty_and_separator_only_paths_have_no_usable_base_name() {
        assert_eq!(convolution_ir_display_name(""), "");
        assert_eq!(convolution_ir_display_name("/"), "/");
    }
}
