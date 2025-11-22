//
//  HeadScannerFFI.h
//  HeadScanner iOS Bridge
//
//  C API for Rust head-scanner library
//

#ifndef HeadScannerFFI_h
#define HeadScannerFFI_h

#include <stdint.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

// Opaque types
typedef struct Scanner Scanner;
typedef struct ScanGuidance ScanGuidance;
typedef struct Mesh Mesh;

// Result codes
typedef enum {
    SCANNER_OK = 0,
    SCANNER_ERROR = 1,
    SCANNER_INVALID_INPUT = 2,
    SCANNER_IO_ERROR = 3,
} ScannerResultCode;

// Point3D
typedef struct {
    float x;
    float y;
    float z;
} Point3D;

// Quaternion for rotation
typedef struct {
    float x;
    float y;
    float z;
    float w;
} Quaternion;

// Camera pose
typedef struct {
    Point3D position;
    Quaternion rotation;
} CameraPose;

// Quality metrics
typedef struct {
    float coverage;
    float angular_coverage;
    float point_density;
    float blur_score;
} QualityMetrics;

// Head region enum
typedef enum {
    REGION_FRONT = 0,
    REGION_LEFT = 1,
    REGION_RIGHT = 2,
    REGION_BACK = 3,
    REGION_TOP = 4,
    REGION_BOTTOM = 5,
    REGION_FRONT_LEFT = 6,
    REGION_FRONT_RIGHT = 7,
    REGION_BACK_LEFT = 8,
    REGION_BACK_RIGHT = 9,
    REGION_TOP_FRONT = 10,
} HeadRegion;

// Scanner lifecycle
Scanner* scanner_new(void);
void scanner_free(Scanner* scanner);

// Frame processing
ScannerResultCode scanner_process_frame(
    Scanner* scanner,
    const uint8_t* rgb_data,
    const float* depth_data,
    uint32_t width,
    uint32_t height,
    const CameraPose* pose
);

// Scan guidance
ScanGuidance* scanner_get_guidance(Scanner* scanner);
void guidance_free(ScanGuidance* guidance);

ScannerResultCode guidance_update_pose(
    ScanGuidance* guidance,
    const CameraPose* pose
);

QualityMetrics guidance_get_metrics(const ScanGuidance* guidance);

bool guidance_is_region_covered(
    const ScanGuidance* guidance,
    HeadRegion region
);

HeadRegion guidance_get_next_region(const ScanGuidance* guidance);

// Mesh reconstruction
Mesh* scanner_get_mesh(Scanner* scanner);
void mesh_free(Mesh* mesh);

uint32_t mesh_vertex_count(const Mesh* mesh);
uint32_t mesh_triangle_count(const Mesh* mesh);

ScannerResultCode mesh_export_obj(
    const Mesh* mesh,
    const char* path
);

// SOFA generation
ScannerResultCode scanner_generate_sofa(
    const Mesh* mesh,
    const char* output_path,
    float sample_rate,
    uint32_t azimuth_resolution,
    uint32_t elevation_resolution,
    float distance
);

// Error handling
const char* scanner_last_error(void);

#ifdef __cplusplus
}
#endif

#endif /* HeadScannerFFI_h */
