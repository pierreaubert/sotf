use std::path::PathBuf;

#[cfg(any(target_os = "macos", test))]
pub(super) fn macos_home_dir_from_env(
    home: Option<&std::ffi::OsStr>,
    cf_fixed_user_home: Option<&std::ffi::OsStr>,
    app_sandbox_container_id: Option<&std::ffi::OsStr>,
) -> Option<PathBuf> {
    let sandbox_id = app_sandbox_container_id
        .and_then(|id| id.to_str())
        .filter(|id| !id.is_empty());

    if sandbox_id.is_some()
        && let Some(fixed_home) = cf_fixed_user_home
        && !fixed_home.is_empty()
    {
        return Some(PathBuf::from(fixed_home));
    }

    let home = home.filter(|home| !home.is_empty()).map(PathBuf::from)?;

    if let Some(sandbox_id) = sandbox_id {
        let container_data_suffix = PathBuf::from("Library")
            .join("Containers")
            .join(sandbox_id)
            .join("Data");

        if home.ends_with(&container_data_suffix) {
            Some(home)
        } else {
            Some(home.join(container_data_suffix))
        }
    } else {
        Some(home)
    }
}

#[cfg(target_os = "macos")]
pub(super) fn macos_home_dir() -> Option<PathBuf> {
    macos_home_dir_from_env(
        std::env::var_os("HOME").as_deref(),
        std::env::var_os("CFFIXED_USER_HOME").as_deref(),
        std::env::var_os("APP_SANDBOX_CONTAINER_ID").as_deref(),
    )
}
