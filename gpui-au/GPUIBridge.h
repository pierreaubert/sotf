//
//  GPUIBridge.h
//  SOTF GPUI Audio Unit Bridge
//
//  FFI interface for embedding GPUI UI in Audio Unit plugins.
//  Based on the JUCE pattern of extracting native NSView from UI framework.
//

#ifndef GPUI_BRIDGE_H
#define GPUI_BRIDGE_H

#include <stdint.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

// =============================================================================
// Opaque Types
// =============================================================================

/// Opaque handle to GPUI embedded view
typedef struct GPUIEmbeddedView GPUIEmbeddedView;

// =============================================================================
// C-compatible Data Structures
// =============================================================================

/// EQ Filter representation (C-compatible)
typedef struct {
    double frequency;   // Hz (20-20000)
    double q;           // Quality factor (0.1-10.0)
    double gain_db;     // Gain in decibels (-20 to +20)
    int32_t filter_type; // 0=Peak, 1=LowShelf, 2=HighShelf, 3=Lowpass, 4=Highpass
} CEQFilter;

// =============================================================================
// Lifecycle Functions
// =============================================================================

/// Create a new GPUI embedded view
///
/// Creates a GPUI context and window without calling Application::run(),
/// allowing integration with Audio Unit's main thread.
///
/// @param width Initial width in pixels
/// @param height Initial height in pixels
/// @return Opaque pointer to GPUIEmbeddedView, or NULL on failure
GPUIEmbeddedView* gpui_view_create(uint32_t width, uint32_t height);

/// Destroy a GPUI embedded view
///
/// Cleans up GPUI resources and deallocates the view.
///
/// @param view The view to destroy (can be NULL)
void gpui_view_destroy(GPUIEmbeddedView* view);

// =============================================================================
// View Management
// =============================================================================

/// Get the native NSView* from GPUI window
///
/// Extracts the Cocoa NSView backing the GPUI window. This NSView can be
/// embedded in the AU view controller using standard Cocoa APIs.
///
/// The returned pointer remains valid as long as the GPUIEmbeddedView exists.
/// The caller must NOT deallocate or retain it.
///
/// @param view The GPUI embedded view
/// @return Opaque pointer to NSView (id), or NULL if GPUI unavailable
void* gpui_view_get_native_view(GPUIEmbeddedView* view);

/// Check if GPUI view is available
///
/// Returns true if the GPUI view was successfully created with a native view.
/// If false, the AU should use its fallback placeholder UI (e.g., SwiftUI or plain AppKit).
///
/// @param view The GPUI embedded view
/// @return true if native view is available, false otherwise
bool gpui_view_is_available(GPUIEmbeddedView* view);

/// Update view size
///
/// Notifies GPUI of a resize event from the AU host.
///
/// @param view The GPUI embedded view
/// @param width New width in pixels
/// @param height New height in pixels
void gpui_view_set_size(GPUIEmbeddedView* view, uint32_t width, uint32_t height);

// =============================================================================
// Parameter Updates
// =============================================================================

/// Update EQ filter parameters
///
/// Called when AU parameters change (e.g., from host automation).
/// Triggers UI re-render with the new filter state.
///
/// @param view The GPUI embedded view
/// @param filters Array of EQ filters
/// @param count Number of filters in the array
void gpui_view_set_filters(
    GPUIEmbeddedView* view,
    const CEQFilter* filters,
    size_t count
);

/// Get EQ filter parameters
///
/// Copies current filter state from GPUI to the provided buffer.
/// Used for bidirectional sync when UI changes filters.
///
/// @param view The GPUI embedded view
/// @param filters Output buffer for EQ filters
/// @param max_count Maximum number of filters to copy
/// @return Number of filters actually copied
size_t gpui_view_get_filters(
    GPUIEmbeddedView* view,
    CEQFilter* filters,
    size_t max_count
);

// =============================================================================
// Input Handling
// =============================================================================

/// Handle mouse events
///
/// Forwards mouse events from NSView to GPUI's input system.
///
/// @param view The GPUI embedded view
/// @param x Mouse X position in view coordinates
/// @param y Mouse Y position in view coordinates
/// @param event_type 0=MouseDown, 1=MouseDrag, 2=MouseUp
void gpui_view_mouse_event(
    GPUIEmbeddedView* view,
    float x,
    float y,
    int32_t event_type
);

#ifdef __cplusplus
}
#endif

#endif // GPUI_BRIDGE_H
