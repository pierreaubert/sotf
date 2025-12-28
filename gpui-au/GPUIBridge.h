//
//  GPUIBridge.h
//  SOTF Audio Unit Plugin View Bridge
//
//  FFI interface for embedding Rust Metal UI in Audio Unit plugins.
//  Provides EQ visualization with interactive band controls.
//

#ifndef GPUI_BRIDGE_H
#define GPUI_BRIDGE_H

#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

// =============================================================================
// Opaque Types
// =============================================================================

/// Opaque handle to AU plugin view
typedef struct AUPluginView AUPluginView;

// =============================================================================
// C-compatible Data Structures
// =============================================================================

/// EQ Band representation (C-compatible)
typedef struct {
    int32_t filter_type;  // 0=Peak, 1=LowShelf, 2=HighShelf, 3=LowPass, 4=HighPass
    float frequency;      // Hz (20-20000)
    float gain_db;        // Gain in decibels (-24 to +24)
    float q;              // Quality factor (0.1-10.0)
    bool enabled;         // Whether band is active
} CAUEQBand;

// =============================================================================
// Lifecycle Functions
// =============================================================================

/// Create a new AU plugin view
///
/// Creates a Metal-backed NSView with EQ visualization.
/// Must be called from the main thread.
///
/// @param width Initial width in pixels
/// @param height Initial height in pixels
/// @return Opaque pointer to AUPluginView, or NULL on failure
AUPluginView* au_plugin_view_create(uint32_t width, uint32_t height);

/// Destroy an AU plugin view
///
/// Cleans up Metal resources and deallocates the view.
///
/// @param view The view to destroy (can be NULL)
void au_plugin_view_destroy(AUPluginView* view);

// =============================================================================
// View Management
// =============================================================================

/// Get the native NSView* from the plugin view
///
/// Returns the Cocoa NSView backing the plugin UI. This NSView can be
/// embedded in the AU view controller using standard Cocoa APIs.
///
/// The returned pointer remains valid as long as the AUPluginView exists.
/// The caller must NOT deallocate or release it.
///
/// @param view The plugin view
/// @return Opaque pointer to NSView (id), or NULL if view is invalid
void* au_plugin_view_get_native(const AUPluginView* view);

/// Request a redraw of the view
///
/// Marks the view as needing display. Call this after updating parameters
/// or when the view needs to refresh.
///
/// @param view The plugin view
void au_plugin_view_set_needs_display(const AUPluginView* view);

// =============================================================================
// Parameter Updates
// =============================================================================

/// Set EQ band parameters
///
/// Updates the EQ visualization with new band settings.
/// Called when AU parameters change (e.g., from host automation).
///
/// @param view The plugin view
/// @param bands Array of EQ bands
/// @param count Number of bands in the array
void au_plugin_view_set_bands(
    AUPluginView* view,
    const CAUEQBand* bands,
    size_t count
);

/// Get EQ band parameters
///
/// Copies current band state from the view to the provided buffer.
/// Used for bidirectional sync when UI changes band values.
///
/// @param view The plugin view
/// @param bands Output buffer for EQ bands
/// @param max_count Maximum number of bands to copy
/// @return Number of bands actually copied
size_t au_plugin_view_get_bands(
    const AUPluginView* view,
    CAUEQBand* bands,
    size_t max_count
);

#ifdef __cplusplus
}
#endif

#endif // GPUI_BRIDGE_H
