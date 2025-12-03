//! WGSL shaders for 3D surface rendering

/// Common struct definitions shared by all shaders
pub const COMMON_DEFINITIONS: &str = r#"
struct Uniforms {
    view_proj: mat4x4<f32>,
    model: mat4x4<f32>,
    light_dir: vec3<f32>,
    _pad1: f32,
    ambient: f32,
    diffuse: f32,
    z_min: f32,
    z_max: f32,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) value: f32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_normal: vec3<f32>,
    @location(1) normalized_value: f32,
}
"#;

/// Vertex shader for surface rendering
pub const SURFACE_VERTEX_SHADER: &str = r#"
@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    let world_pos = uniforms.model * vec4<f32>(in.position, 1.0);
    out.clip_position = uniforms.view_proj * world_pos;

    // Transform normal to world space (assuming uniform scaling)
    let normal_matrix = mat3x3<f32>(
        uniforms.model[0].xyz,
        uniforms.model[1].xyz,
        uniforms.model[2].xyz
    );
    out.world_normal = normalize(normal_matrix * in.normal);

    // Pass through normalized value for colormap
    out.normalized_value = in.value;

    return out;
}
"#;

/// Fragment shader for surface rendering with colormap
pub const SURFACE_FRAGMENT_SHADER: &str = r#"

// Viridis colormap approximation
fn viridis(t: f32) -> vec3<f32> {
    let c0 = vec3<f32>(0.2777, 0.0054, 0.3340);
    let c1 = vec3<f32>(0.1050, 0.6387, 0.2383);
    let c2 = vec3<f32>(-0.3308, 0.3143, 0.5287);
    let c3 = vec3<f32>(-4.6342, -5.7991, -19.3324);
    let c4 = vec3<f32>(6.2282, 14.1799, 56.6905);
    let c5 = vec3<f32>(4.7763, -13.7451, -65.3530);
    let c6 = vec3<f32>(-5.4354, 4.6456, 26.3124);

    let t2 = t * t;
    let t3 = t2 * t;
    let t4 = t3 * t;
    let t5 = t4 * t;
    let t6 = t5 * t;

    return clamp(c0 + c1*t + c2*t2 + c3*t3 + c4*t4 + c5*t5 + c6*t6, vec3<f32>(0.0), vec3<f32>(1.0));
}

// Plasma colormap approximation
fn plasma(t: f32) -> vec3<f32> {
    let c0 = vec3<f32>(0.0504, 0.0298, 0.5280);
    let c1 = vec3<f32>(2.0280, -0.3996, -0.1361);
    let c2 = vec3<f32>(-2.1285, 1.3971, -1.8103);
    let c3 = vec3<f32>(-10.2107, 6.8536, 18.8406);
    let c4 = vec3<f32>(33.6908, -21.2851, -41.8887);
    let c5 = vec3<f32>(-38.8641, 25.8915, 35.6632);
    let c6 = vec3<f32>(12.8861, -7.9772, -11.5408);

    let t2 = t * t;
    let t3 = t2 * t;
    let t4 = t3 * t;
    let t5 = t4 * t;
    let t6 = t5 * t;

    return clamp(c0 + c1*t + c2*t2 + c3*t3 + c4*t4 + c5*t5 + c6*t6, vec3<f32>(0.0), vec3<f32>(1.0));
}

// Inferno colormap approximation
fn inferno(t: f32) -> vec3<f32> {
    let c0 = vec3<f32>(0.0002, 0.0016, 0.0139);
    let c1 = vec3<f32>(0.1260, 0.4023, 1.3241);
    let c2 = vec3<f32>(1.1661, 0.0868, -2.1073);
    let c3 = vec3<f32>(-1.0127, 2.0841, 2.4048);
    let c4 = vec3<f32>(-8.8174, 0.1567, -2.5439);
    let c5 = vec3<f32>(17.5174, -4.5424, 0.8282);
    let c6 = vec3<f32>(-9.5028, 3.3025, 0.0987);

    let t2 = t * t;
    let t3 = t2 * t;
    let t4 = t3 * t;
    let t5 = t4 * t;
    let t6 = t5 * t;

    return clamp(c0 + c1*t + c2*t2 + c3*t3 + c4*t4 + c5*t5 + c6*t6, vec3<f32>(0.0), vec3<f32>(1.0));
}

// Turbo colormap (Google's improved rainbow)
fn turbo(t: f32) -> vec3<f32> {
    let r = clamp(0.13572 + t * (4.6153 + t * (-42.6592 + t * (138.5676 + t * (-152.3494 + t * 59.2859)))), 0.0, 1.0);
    let g = clamp(0.09140 + t * (2.2537 + t * (0.6487 + t * (-23.3910 + t * (38.3522 - t * 18.0858)))), 0.0, 1.0);
    let b = clamp(0.10667 + t * (12.5925 + t * (-60.5820 + t * (109.7316 + t * (-88.2949 + t * 26.7236)))), 0.0, 1.0);
    return vec3<f32>(r, g, b);
}

// Cool-warm diverging colormap
fn coolwarm(t: f32) -> vec3<f32> {
    // Blue (cool) to red (warm) through white
    let mid = 0.5;
    if (t < mid) {
        let s = t / mid;
        return mix(vec3<f32>(0.23, 0.30, 0.75), vec3<f32>(0.87, 0.87, 0.87), s);
    } else {
        let s = (t - mid) / (1.0 - mid);
        return mix(vec3<f32>(0.87, 0.87, 0.87), vec3<f32>(0.71, 0.02, 0.15), s);
    }
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Calculate lighting
    let normal = normalize(in.world_normal);
    let light_dir = normalize(uniforms.light_dir);

    let ndotl = max(dot(normal, light_dir), 0.0);
    let lighting = uniforms.ambient + uniforms.diffuse * ndotl;

    // Apply colormap (using viridis by default)
    let base_color = viridis(in.normalized_value);

    // Combine lighting with color
    let final_color = base_color * lighting;

    return vec4<f32>(final_color, 1.0);
}
"#;

/// Simple wireframe shader (uses same vertex shader)
pub const WIREFRAME_FRAGMENT_SHADER: &str = r#"
@fragment
fn fs_wireframe(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(0.2, 0.2, 0.2, 1.0);
}
"#;

/// Combined shader source
pub fn combined_shader() -> String {
    format!(
        "{}\n{}\n{}\n{}",
        COMMON_DEFINITIONS,
        SURFACE_VERTEX_SHADER,
        SURFACE_FRAGMENT_SHADER,
        WIREFRAME_FRAGMENT_SHADER
    )
}
