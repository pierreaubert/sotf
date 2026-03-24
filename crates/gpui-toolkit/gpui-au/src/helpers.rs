//! Shared helpers for the gpui-au crate.

use objc::runtime::Object;

/// Log via NSLog (always visible in Console.app, unlike Rust's log crate).
/// The message must be a null-terminated string.
pub(crate) fn nslog(msg: &str) {
    use objc::{class, msg_send, sel, sel_impl};
    unsafe {
        let ns_string: *mut Object =
            msg_send![class!(NSString), stringWithUTF8String: msg.as_ptr()];
        #[link(name = "Foundation", kind = "framework")]
        unsafe extern "C" {
            fn NSLog(format: *mut Object, ...);
        }
        let fmt: *mut Object =
            msg_send![class!(NSString), stringWithUTF8String: c"%@".as_ptr()];
        NSLog(fmt, ns_string);
    }
}
