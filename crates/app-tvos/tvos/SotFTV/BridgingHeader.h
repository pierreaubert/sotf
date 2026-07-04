#ifndef BridgingHeader_h
#define BridgingHeader_h

// ── Rust → Swift (called from Rust staticlib) ──────────────────────────────

void sotf_tvos_start(void);

// ── GPUI iOS/tvOS platform FFI ─────────────────────────────────────────────

void gpui_ios_request_frame(void *window_ptr);
void gpui_ios_request_current_frame(void);
void *gpui_ios_get_window(void);

// Lifecycle
void gpui_ios_will_enter_foreground(void *app_ptr);
void gpui_ios_did_become_active(void *app_ptr);
void gpui_ios_will_resign_active(void *app_ptr);
void gpui_ios_did_enter_background(void *app_ptr);
void gpui_ios_will_terminate(void *app_ptr);

// ── Swift → Rust (audio lifecycle) ─────────────────────────────────────────

void sotf_tvos_audio_interrupted(bool began);
void sotf_tvos_remote_play(void);
void sotf_tvos_remote_pause(void);
void sotf_tvos_remote_toggle_play_pause(void);

#endif /* BridgingHeader_h */
