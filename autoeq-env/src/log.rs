// Logging macro for consistent timestamp format
#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {{
        let now = chrono::Local::now();
        eprintln!("{}  DEBUG [audio_streaming] {}", now.format("%Y-%m-%d %H:%M:%S%.6f"), format!($($arg)*));
    }};
}

#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {{
        let now = chrono::Local::now();
        eprintln!("{}  INFO  [audio_streaming] {}", now.format("%Y-%m-%d %H:%M:%S%.6f"), format!($($arg)*));
    }};
}

#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {{
        let now = chrono::Local::now();
        eprintln!("{}  WARN  [audio_streaming] {}", now.format("%Y-%m-%d %H:%M:%S%.6f"), format!($($arg)*));
    }};
}

#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {{
        let now = chrono::Local::now();
        eprintln!("{}  ERROR [audio_streaming] {}", now.format("%Y-%m-%d %H:%M:%S%.6f"), format!($($arg)*));
    }};
}
