// BridgingHeader.h
// C FFI interface to Rust audio plugins

#ifndef BridgingHeader_h
#define BridgingHeader_h

#include <stdint.h>
#include <stddef.h>

// GPUI Bridge for Rust Metal UI
#include "GPUIBridge.h"

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
    const char* id;               // Parameter ID (e.g., "band0_freq")
    const char* name;             // Display name (e.g., "Band 1 Frequency")
    const char* unit;             // Unit string (e.g., "Hz", "dB")
    double min_value;             // Minimum value
    double max_value;             // Maximum value
    double default_value;         // Default value
    uint32_t steps;               // Number of steps (0 = continuous)
} ParameterInfo;

// ============================================================================
// Plugin Lifecycle
// ============================================================================

/// Create a new plugin instance
///
/// @param plugin_type Plugin type name (e.g., "EQ")
/// @param config_json JSON configuration string
/// @param sample_rate Sample rate in Hz
/// @param input_channels Number of input channels
/// @param output_channels Number of output channels
/// @return Plugin handle on success, NULL on failure
PluginHandle* plugin_create(
    const char* plugin_type,
    const char* config_json,
    uint32_t sample_rate,
    size_t input_channels,
    size_t output_channels
);

/// Destroy a plugin instance
///
/// @param handle Plugin handle (must not be NULL)
void plugin_destroy(PluginHandle* handle);

/// Reset plugin state (clear buffers, reset filters)
///
/// @param handle Plugin handle
/// @return 0 on success, error code on failure
int plugin_reset(PluginHandle* handle);

// ============================================================================
// Audio Processing
// ============================================================================

/// Process audio samples
///
/// @param handle Plugin handle
/// @param input Interleaved input samples
/// @param output Interleaved output buffer
/// @param num_frames Number of frames to process
/// @return 0 on success, error code on failure
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
///
/// @param handle Plugin handle
/// @return Number of parameters
int plugin_get_parameter_count(const PluginHandle* handle);

/// Get parameter info by index
///
/// @param handle Plugin handle
/// @param index Parameter index
/// @return Pointer to parameter info, NULL if index out of bounds
const ParameterInfo* plugin_get_parameter_info(
    const PluginHandle* handle,
    size_t index
);

/// Set a parameter value (normalized 0.0-1.0)
///
/// @param handle Plugin handle
/// @param param_id Parameter ID string
/// @param normalized_value Normalized value (0.0 = min, 1.0 = max)
/// @return 0 on success, error code on failure
int plugin_set_parameter(
    PluginHandle* handle,
    const char* param_id,
    double normalized_value
);

/// Get a parameter value (normalized 0.0-1.0)
///
/// @param handle Plugin handle
/// @param param_id Parameter ID string
/// @return Normalized value, -1.0 on error
double plugin_get_parameter(
    const PluginHandle* handle,
    const char* param_id
);

// ============================================================================
// Plugin Information
// ============================================================================

/// Get plugin information as JSON string
///
/// @param handle Plugin handle
/// @return JSON string (caller must free with plugin_free_string())
char* plugin_get_info_json(const PluginHandle* handle);

/// Free a string returned by the plugin
///
/// @param s String pointer
void plugin_free_string(char* s);

/// Get last error message
///
/// @return Error message string, NULL if no error
const char* plugin_get_last_error(void);

#ifdef __cplusplus
}
#endif

#endif /* BridgingHeader_h */
