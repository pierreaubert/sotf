use super::misc::is_project_root;
use std::path::{Path, PathBuf};

pub(super) fn find_project_root(root_override: Option<PathBuf>) -> PathBuf {
    if let Some(root) = root_override {
        if is_project_root(&root) {
            return root;
        }
        panic!(
            "--root must point at the SOTF workspace root (Cargo.toml with [workspace] and site/src/content/docs), got {}",
            root.display()
        );
    }

    if let Ok(current) = std::env::current_dir()
        && let Some(root) = find_project_root_from(&current)
    {
        return root;
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if let Some(root) = find_project_root_from(&manifest_dir) {
        return root;
    }

    panic!(
        "Could not find SOTF workspace root. Run from the workspace root or pass --root <path>."
    );
}

pub(super) fn find_project_root_from(start: &Path) -> Option<PathBuf> {
    let mut dir = start.to_path_buf();
    loop {
        if is_project_root(&dir) {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}
