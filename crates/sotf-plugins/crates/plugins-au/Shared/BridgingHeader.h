// BridgingHeader.h
// C FFI interface to Rust audio plugins

#ifndef BridgingHeader_h
#define BridgingHeader_h

#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>

// GPUI AU rendering FFI
#include "gpui_au_ffi.h"

#ifdef __cplusplus
extern "C" {
#endif

// ============================================================================
// Opaque Types
// ============================================================================

/// Opaque handle to plugin instance (managed by Rust)
typedef struct PluginHandle PluginHandle;

// ============================================================================
// Error Codes
// ============================================================================

typedef enum {
    PluginError_Success = 0,
    PluginError_InvalidHandle = -1,
    PluginError_InvalidParameter = -2,
    PluginError_NullPointer = -3,
    PluginError_InvalidUtf8 = -4,
    PluginError_PluginCreationFailed = -5,
    PluginError_ProcessingFailed = -6,
    PluginError_InitializationFailed = -7,
    PluginError_InvalidConfig = -8,
    PluginError_UnknownError = -99,
} PluginError;

// ============================================================================
// Parameter Info
// ============================================================================

typedef struct {
    const char* id;               // Parameter ID (e.g., "threshold_db")
    const char* name;             // Display name (e.g., "Threshold")
    const char* unit;             // Unit string (e.g., "Hz", "dB")
    double min_value;             // Minimum value
    double max_value;             // Maximum value
    double default_value;         // Default value
    uint32_t steps;               // Number of steps (0 = continuous)
    bool logarithmic;             // Whether to use logarithmic scaling
} ParameterInfo;

// ============================================================================
// Plugin Lifecycle
// ============================================================================

/// Create a new plugin instance
PluginHandle* plugin_create(
    const char* plugin_type,
    const char* config_json,
    uint32_t sample_rate,
    size_t input_channels,
    size_t output_channels
);

/// Destroy a plugin instance
void plugin_destroy(PluginHandle* handle);

/// Reset plugin state
int plugin_reset(PluginHandle* handle);

// ============================================================================
// Audio Processing
// ============================================================================

/// Process interleaved audio samples
int plugin_process(
    PluginHandle* handle,
    const float* input,
    float* output,
    size_t num_frames
);

// ============================================================================
// Parameter Management
// ============================================================================

/// Get the number of parameters
int plugin_get_parameter_count(const PluginHandle* handle);

/// Get parameter info by index
const ParameterInfo* plugin_get_parameter_info(const PluginHandle* handle, size_t index);

/// Set a parameter value (normalized 0.0-1.0)
int plugin_set_parameter(PluginHandle* handle, const char* param_id, double normalized_value);

/// Get a parameter value (normalized 0.0-1.0)
double plugin_get_parameter(const PluginHandle* handle, const char* param_id);

// ============================================================================
// Plugin Information
// ============================================================================

/// Get plugin information as JSON string (caller must free with plugin_free_string)
char* plugin_get_info_json(const PluginHandle* handle);

/// Free a string returned by the plugin
void plugin_free_string(char* s);

/// Get last error message (NULL if no error)
const char* plugin_get_last_error(void);

// ============================================================================
// State Save/Load
// ============================================================================

/// Save plugin state to JSON bytes (caller must free with plugin_free_state)
uint8_t* plugin_save_state(const PluginHandle* handle, size_t* out_len);

/// Load plugin state from JSON bytes
int plugin_load_state(PluginHandle* handle, const uint8_t* data, size_t len);

/// Free state buffer returned by plugin_save_state
void plugin_free_state(uint8_t* data, size_t len);

/// Get list of available plugin types as JSON array string (caller must free with plugin_free_string)
char* plugin_available_types(void);

#ifdef __cplusplus
}
#endif

#endif /* BridgingHeader_h */
