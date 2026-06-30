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
