#ifndef BridgingHeader_h
#define BridgingHeader_h

#include <stdbool.h>

// Rust FFI functions exported by the sotf_ios staticlib + gpui-ios

// Initialize and run the GPUI SotF player app
void sotf_ios_start(void);

// Frame rendering — call from CADisplayLink
void gpui_ios_request_frame(void *window_ptr);

// Get the active GPUI window pointer
void *gpui_ios_get_window(void);

// GPUI-in-SwiftUI hosting
void *gpui_ios_attach_to_view(void *parent);
void gpui_ios_detach_view(void *window_ptr);

// Native platform views
typedef void *(*GPUIPlatformViewCreateCallback)(const char *view_type,
                                                const char *creation_params);
typedef void (*GPUIPlatformViewUpdateBoundsCallback)(void *view, float x,
                                                     float y, float width,
                                                     float height);
typedef void (*GPUIPlatformViewSetBoolCallback)(void *view, bool value);
typedef void (*GPUIPlatformViewSetZIndexCallback)(void *view, int z_index);
typedef void (*GPUIPlatformViewDisposeCallback)(void *view);

bool gpui_ios_register_platform_view_factory(
    const char *view_type, int kind, GPUIPlatformViewCreateCallback create,
    GPUIPlatformViewUpdateBoundsCallback update_bounds,
    GPUIPlatformViewSetBoolCallback set_visible,
    GPUIPlatformViewSetZIndexCallback set_z_index,
    GPUIPlatformViewDisposeCallback dispose);

// Debug instrumentation
bool gpui_ios_begin_metal_capture(const char *label);
void gpui_ios_end_metal_capture(void);

// Touch event forwarding
void gpui_ios_handle_touch(void *window_ptr, void *touch_ptr, void *event_ptr);
bool gpui_ios_handle_pencil_hover(float x, float y, float altitude_angle,
                                  float azimuth_angle, float distance,
                                  double timestamp_seconds);

// Lifecycle
void gpui_ios_will_enter_foreground(void *app_ptr);
void gpui_ios_did_become_active(void *app_ptr);
void gpui_ios_will_resign_active(void *app_ptr);
void gpui_ios_did_enter_background(void *app_ptr);
void gpui_ios_will_terminate(void *app_ptr);

// Keyboard
void gpui_ios_show_keyboard(void *window_ptr);
void gpui_ios_hide_keyboard(void *window_ptr);
void gpui_ios_handle_text_input(void *window_ptr, void *text_ptr);
void gpui_ios_handle_key_event(void *window_ptr, unsigned int key_code,
                                unsigned int modifiers, _Bool is_key_down);

// File import — called by Swift DocumentPicker → Rust
void sotf_ios_files_imported(const char *paths_json);

// Music directory — called by Rust to get the iOS sandbox music path
const char *sotf_ios_get_music_directory(void);

// Audio lifecycle — called by Swift AudioManager → Rust
void sotf_ios_audio_interrupted(_Bool began);
void sotf_ios_audio_route_changed(void);

// Remote commands — called by Swift MPRemoteCommandCenter → Rust
void sotf_ios_remote_play(void);
void sotf_ios_remote_pause(void);
void sotf_ios_remote_toggle_play_pause(void);
void sotf_ios_remote_next_track(void);
void sotf_ios_remote_prev_track(void);
void sotf_ios_remote_seek(double position);

// Now Playing — called by Rust → Swift to update lock screen info
// (implemented as @_cdecl in AudioManager.swift)

#endif
