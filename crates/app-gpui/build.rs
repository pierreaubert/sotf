fn main() {
    // Embed the SotF icon as a Win32 resource so the taskbar and title bar show it.
    // GPUI's Windows platform loads resource ID 1 via LoadImageW.
    #[cfg(target_os = "windows")]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/sotf.ico");
        if let Err(e) = res.compile() {
            eprintln!("cargo:warning=Failed to compile Windows icon resource: {e}");
        }
    }
}
