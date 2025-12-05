//! WGSL shaders for 3D surface rendering

/// Common struct definitions shared by all shaders
pub const COMMON_DEFINITIONS: &str = r#"
struct Uniforms {
    view_proj: mat4x4<f32>,
    model: mat4x4<f32>,
    light_dir: vec3<f32>,
    colormap: f32,
    ambient: f32,
    diffuse: f32,
    opacity: f32,
    z_min: f32,
    x_min_log: f32,
    x_range_log: f32,
    is_log_x: f32,
    padding: f32,
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
    @location(2) world_pos: vec3<f32>,
}
"#;

/// Vertex shader for surface rendering
pub const SURFACE_VERTEX_SHADER: &str = r#"
@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    let world_pos = uniforms.model * vec4<f32>(in.position, 1.0);
    out.clip_position = uniforms.view_proj * world_pos;
    out.world_pos = world_pos.xyz;

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

fn get_color(t: f32, map_id: f32) -> vec3<f32> {
    if (map_id < 0.5) {
        return viridis(t);
    } else if (map_id < 1.5) {
        return plasma(t);
    } else if (map_id < 2.5) {
        return inferno(t);
    } else if (map_id < 3.5) {
        return turbo(t);
    } else {
        return coolwarm(t);
    }
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Calculate lighting
    let normal = normalize(in.world_normal);
    let light_dir = normalize(uniforms.light_dir);

    // Two-sided lighting
    let ndotl = abs(dot(normal, light_dir));
    let lighting = uniforms.ambient + uniforms.diffuse * ndotl;

    // Apply colormap
    let base_color = get_color(in.normalized_value, uniforms.colormap);

    // Combine lighting with color
    let final_color = base_color * lighting;
    
    // Isolines on surface
    // Every 3dB. Assuming normalized value 0..1 maps to range (e.g. 50dB).
    // 3dB is approx 0.06 normalized units.
    let step = 0.06;
    let line_width = 0.001;
    let feather = 0.0005;
    
    let dist = abs(fract(in.normalized_value / step) - 0.5) * step;
    
    var color = final_color;
    
    // Anti-aliased line
    let line_alpha = 1.0 - smoothstep(line_width - feather, line_width + feather, dist);
    
    if (line_alpha > 0.0) {
        // Blend black line
        color = mix(color, vec3<f32>(0.0, 0.0, 0.0), line_alpha * 0.5);
    }

    return vec4<f32>(color, uniforms.opacity);
}
"#;

/// Simple wireframe shader (uses same vertex shader)
pub const WIREFRAME_FRAGMENT_SHADER: &str = r#"
@fragment
fn fs_wireframe(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(0.2, 0.2, 0.2, 0.5);
}
"#;

/// Vertex shader for projection/isolines
pub const PROJECTION_VERTEX_SHADER: &str = r#"
@vertex
fn vs_projection(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // Flatten to Y plane (bottom/floor)
    // Position is (x=Freq, y=SPL, z=Angle)
    // We want to project to the floor (y = -0.5)
    // Offset slightly to -0.499 to avoid Z-fighting with grid floor
    let flat_pos = vec3<f32>(in.position.x, -0.499, in.position.z);
    let world_pos = uniforms.model * vec4<f32>(flat_pos, 1.0);
    out.clip_position = uniforms.view_proj * world_pos;
    out.world_pos = world_pos.xyz;
    out.world_normal = vec3<f32>(0.0, 1.0, 0.0); // Normal points up
    out.normalized_value = in.value;

    return out;
}
"#;

/// Fragment shader for isolines
pub const PROJECTION_FRAGMENT_SHADER: &str = r#"
@fragment
fn fs_projection(in: VertexOutput) -> @location(0) vec4<f32> {
    let value = in.normalized_value;
    // Isolines every 3dB
    let step = 0.06;
    let line_width = 0.001;
    let feather = 0.0005;
    
    let dist = abs(fract(value / step) - 0.5) * step;
    
    // Anti-aliased line
    let line_alpha = 1.0 - smoothstep(line_width - feather, line_width + feather, dist);
    
    if (line_alpha > 0.0) {
        return vec4<f32>(0.0, 0.0, 0.0, line_alpha * 0.8); // Black isolines
    } else {
        discard; // Transparent between lines
    }
    return vec4<f32>(0.0, 0.0, 0.0, 0.0);
}
"#;

/// Fragment shader for grid box
pub const GRID_FRAGMENT_SHADER: &str = r#"
@fragment
fn fs_grid(in: VertexOutput) -> @location(0) vec4<f32> {
    let pos = in.world_pos;

    // Determine which face we are on
    // X=-1, X=1, Y=-0.5, Y=0.5, Z=-1, Z=1

    var u = 0.0;
    var v = 0.0;
    var is_face = false;

    let eps = 0.01;

    if (abs(pos.x + 1.0) < eps || abs(pos.x - 1.0) < eps) {
        // YZ plane (Left/Right)
        // Map Z [-1, 1] to [0, 1]
        u = (pos.z + 1.0) * 0.5;
        v = pos.y + 0.5; // Y is [-0.5, 0.5] -> [0, 1]
        is_face = true;
    } else if (abs(pos.y + 0.5) < eps || abs(pos.y - 0.5) < eps) {
        // XZ plane (Bottom/Top)
        // Map X, Z [-1, 1] to [0, 1]
        u = (pos.x + 1.0) * 0.5;
        v = (pos.z + 1.0) * 0.5;
        is_face = true;
    } else if (abs(pos.z + 1.0) < eps || abs(pos.z - 1.0) < eps) {
        // XY plane (Front/Back)
        // Map X [-1, 1] to [0, 1]
        u = (pos.x + 1.0) * 0.5;
        v = pos.y + 0.5;
        is_face = true;
    }

    if (!is_face) {
        discard;
    }

    // Grid lines
    let major_steps = 5.0;
    let minor_steps = 25.0;
    
    // Analytic AA using derivatives
    // Compute screen-space change of u and v
    let du = fwidth(u);
    let dv = fwidth(v);
    
    // Line width in UV space (approx 1.5 pixels)
    // We use max(du, dv) as a conservative estimate or individual widths
    let line_width_u = du * 1.0;
    let line_width_v = dv * 1.0;

    // Distance to nearest major grid line (interior lines) will be calculated below

    // Distance to border lines (at u=0, u=1, v=0, v=1)
    // We want a solid distinct border frame
    let dist_u_border = min(u, 1.0 - u);
    let dist_v_border = min(v, 1.0 - v);
    
    // Logarithmic X axis support
    // If is_log_x is true, and we are on a face where U corresponds to X (XY, XZ),
    // we should modify u-grid lines.
    // Faces:
    // XY (Front/Back): u maps to X.
    // XZ (Bottom/Top): u maps to X.
    // YZ (Left/Right): u maps to Z. Log X doesn't affect Z lines.
    
    var is_log_u = false;
    if (uniforms.is_log_x > 0.5) {
        // Check if we are on XY or XZ plane
        // XY: abs(pos.z) ~ 1.0. XZ: abs(pos.y) ~ 0.5.
        if (abs(pos.z) > 0.9 || abs(pos.y) < 0.6) {
             // Wait, logic above:
             // YZ: abs(pos.x) ~ 1.0 -> u is Z. (Safe)
             // XZ: abs(pos.y) ~ 0.5 -> u is X. (Log apply)
             // XY: abs(pos.z) ~ 1.0 -> u is X. (Log apply)
             if (abs(pos.x) < 0.99) {
                 is_log_u = true;
             }
        }
    }

    // Distance to nearest major grid line (interior lines)
    var dist_u_maj = 0.0;
    
    if (is_log_u) {
        // Logarithmic grid lines at decades (10^k)
        // u = (log(x) - log(min)) / (log(max) - log(min))
        // We want lines where x = 1 * 10^k
        // log(x) = k
        // u = (k - log(min)) / range
        // k = u * range + log(min)
        // We want k to be integer.
        
        let k = u * uniforms.x_range_log + uniforms.x_min_log;
        // Distance to nearest integer k
        let k_dist = abs(fract(k + 0.5) - 0.5);
        // Convert distance back to U space?
        // dk/du = range
        // du = dk / range
        dist_u_maj = k_dist / uniforms.x_range_log;
    } else {
        dist_u_maj = abs(fract(u * major_steps + 0.5) - 0.5) / major_steps;
    }

    let dist_v_maj = abs(fract(v * major_steps + 0.5) - 0.5) / major_steps;
    let border_width_u = du * 2.0; // Slightly thicker border
    let border_width_v = dv * 2.0;
    
    // Smoothstep for AA
    let border_u_alpha = 1.0 - smoothstep(border_width_u - du, border_width_u + du, dist_u_border);
    let border_v_alpha = 1.0 - smoothstep(border_width_v - dv, border_width_v + dv, dist_v_border);
    let border_alpha = max(border_u_alpha, border_v_alpha);

    // Minor lines
    let dist_u_min = abs(fract(u * minor_steps + 0.5) - 0.5) / minor_steps;
    let dist_v_min = abs(fract(v * minor_steps + 0.5) - 0.5) / minor_steps;

    var color = vec4<f32>(0.0, 0.0, 0.0, 0.0);

    // Light blue grid color
    let grid_color = vec3<f32>(0.8, 0.85, 0.95);
    let border_color = vec3<f32>(0.0, 0.0, 0.0); // Black border
    
    let major_alpha = 0.8;
    let minor_alpha = 0.4;

    // Anti-aliased grid lines (interior)
    let maj_u_alpha = 1.0 - smoothstep(line_width_u - du, line_width_u + du, dist_u_maj);
    let maj_v_alpha = 1.0 - smoothstep(line_width_v - dv, line_width_v + dv, dist_v_maj);
    let maj_alpha_val = max(maj_u_alpha, maj_v_alpha);

    let min_u_alpha = 1.0 - smoothstep(line_width_u - du, line_width_u + du, dist_u_min);
    let min_v_alpha = 1.0 - smoothstep(line_width_v - dv, line_width_v + dv, dist_v_min);
    let min_alpha_val = max(min_u_alpha, min_v_alpha);

    if (border_alpha > 0.0) {
        // Draw border
        color = vec4<f32>(border_color, border_alpha);
    } else if (maj_alpha_val > 0.0) {
        color = vec4<f32>(grid_color * 0.8, major_alpha * maj_alpha_val);
    } else if (min_alpha_val > 0.0) {
        color = vec4<f32>(grid_color, minor_alpha * min_alpha_val);
    } else {
        // Transparent background
        color = vec4<f32>(1.0, 1.0, 1.0, 0.0);
    }

    return color;
}
"#;

/// Combined shader source
pub fn combined_shader() -> String {
    format!(
        "{}\n{}\n{}\n{}\n{}\n{}\n{}",
        COMMON_DEFINITIONS,
        SURFACE_VERTEX_SHADER,
        SURFACE_FRAGMENT_SHADER,
        WIREFRAME_FRAGMENT_SHADER,
        PROJECTION_VERTEX_SHADER,
        PROJECTION_FRAGMENT_SHADER,
        GRID_FRAGMENT_SHADER
    )
}
