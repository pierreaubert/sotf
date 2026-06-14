/// Helper to get brand image path from device name
pub(super) fn get_brand_image_path(device_name: &str) -> Option<&'static str> {
    let lower_name = device_name.to_lowercase();
    if lower_name.contains("dolby") {
        return Some("brands/dolby-audio.png");
    }
    if lower_name.contains("sotf") || lower_name.contains("virtual audio") {
        return Some("sotf.png");
    }
    if lower_name.contains("mac") || lower_name.contains("apple") {
        return Some("brands/apple-mac-mini.png");
    }
    if lower_name.contains("phonum") || lower_name.contains("beyerdynamic") {
        return Some("brands/bayerdynamic-phonum.jpg");
    }
    if lower_name.contains("blackhole") {
        return Some("brands/blackhole.jpeg");
    }
    if lower_name.contains("focusrite") || lower_name.contains("scarlett") {
        return Some("brands/focusrite.png");
    }
    if lower_name.contains("kef") || lower_name.contains("ls60") {
        return Some("brands/kef-ls60.jpg");
    }
    if lower_name.contains("lg") || lower_name.contains("ultrafine") {
        return Some("brands/lg.png");
    }
    if lower_name.contains("rme") || lower_name.contains("fireface") {
        return Some("brands/rme.jpg");
    }
    if lower_name.contains("adam") {
        return Some("brands/adam.png");
    }
    if lower_name.contains("samsung") {
        return Some("brands/samsung-q9.png");
    }
    if lower_name.contains("usb") {
        return Some("brands/usb.png");
    }
    None
}
