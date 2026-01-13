//! Simple 2D renderer for Audio Unit plugin UI
//!
//! Provides basic primitives for rendering EQ curves, control points,
//! and UI elements using Metal.

use cocoa::base::id;
use metal::foreign_types::ForeignType;
use metal::{
    Buffer, CommandQueue, Device, Library, MTLPixelFormat, MTLPrimitiveType, MTLResourceOptions,
    RenderPipelineDescriptor, RenderPipelineState,
};
use objc::{class, msg_send, sel, sel_impl};
use std::ffi::c_void;
use std::mem;

/// Vertex for 2D rendering with position and color
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Vertex2D {
    pub position: [f32; 2],
    pub color: [f32; 4],
}

/// Simple 2D renderer using Metal
pub struct Renderer2D {
    device: Device,
    command_queue: CommandQueue,
    pipeline_state: RenderPipelineState,
    vertex_buffer: Buffer,
    vertices: Vec<Vertex2D>,
    max_vertices: usize,
}

impl Renderer2D {
    /// Create a new 2D renderer
    pub fn new(device: Device, command_queue: CommandQueue) -> Option<Self> {
        // Compile shaders
        let library = Self::create_shader_library(&device)?;

        // Create pipeline state
        let pipeline_state = Self::create_pipeline_state(&device, &library)?;

        // Create vertex buffer (start with capacity for 4096 vertices)
        let max_vertices = 4096;
        let buffer_size = max_vertices * mem::size_of::<Vertex2D>();
        let vertex_buffer = device.new_buffer(
            buffer_size as u64,
            MTLResourceOptions::CPUCacheModeDefaultCache,
        );

        Some(Self {
            device,
            command_queue,
            pipeline_state,
            vertex_buffer,
            vertices: Vec::with_capacity(max_vertices),
            max_vertices,
        })
    }

    /// Create shader library with vertex and fragment shaders
    fn create_shader_library(device: &Device) -> Option<Library> {
        let shader_source = r#"
            #include <metal_stdlib>
            using namespace metal;

            struct Vertex2D {
                float2 position [[attribute(0)]];
                float4 color [[attribute(1)]];
            };

            struct RasterizerData {
                float4 position [[position]];
                float4 color;
            };

            vertex RasterizerData vertex_main(
                Vertex2D in [[stage_in]],
                constant float2 &viewport_size [[buffer(1)]]
            ) {
                RasterizerData out;
                // Convert from pixel coordinates to clip space (-1 to 1)
                float2 clip_pos = (in.position / viewport_size) * 2.0 - 1.0;
                // Flip Y axis (Metal's origin is top-left in clip space after flip)
                clip_pos.y = -clip_pos.y;
                out.position = float4(clip_pos, 0.0, 1.0);
                out.color = in.color;
                return out;
            }

            fragment float4 fragment_main(RasterizerData in [[stage_in]]) {
                return in.color;
            }
        "#;

        match device.new_library_with_source(shader_source, &metal::CompileOptions::new()) {
            Ok(library) => Some(library),
            Err(e) => {
                log::error!("Failed to compile shaders: {}", e);
                None
            }
        }
    }

    /// Create render pipeline state
    fn create_pipeline_state(device: &Device, library: &Library) -> Option<RenderPipelineState> {
        let vertex_fn = library.get_function("vertex_main", None).ok()?;
        let fragment_fn = library.get_function("fragment_main", None).ok()?;

        let pipeline_desc = RenderPipelineDescriptor::new();
        pipeline_desc.set_vertex_function(Some(&vertex_fn));
        pipeline_desc.set_fragment_function(Some(&fragment_fn));

        // Configure vertex descriptor
        let vertex_desc = metal::VertexDescriptor::new();

        // Position attribute
        let pos_attr = vertex_desc.attributes().object_at(0).unwrap();
        pos_attr.set_format(metal::MTLVertexFormat::Float2);
        pos_attr.set_offset(0);
        pos_attr.set_buffer_index(0);

        // Color attribute
        let color_attr = vertex_desc.attributes().object_at(1).unwrap();
        color_attr.set_format(metal::MTLVertexFormat::Float4);
        color_attr.set_offset(8); // After 2 floats for position
        color_attr.set_buffer_index(0);

        // Layout
        let layout = vertex_desc.layouts().object_at(0).unwrap();
        layout.set_stride(mem::size_of::<Vertex2D>() as u64);
        layout.set_step_function(metal::MTLVertexStepFunction::PerVertex);

        pipeline_desc.set_vertex_descriptor(Some(vertex_desc));

        // Color attachment (BGRA8Unorm for CAMetalLayer)
        let color_attachment = pipeline_desc.color_attachments().object_at(0).unwrap();
        color_attachment.set_pixel_format(MTLPixelFormat::BGRA8Unorm);

        // Enable blending for anti-aliased lines
        color_attachment.set_blending_enabled(true);
        color_attachment.set_source_rgb_blend_factor(metal::MTLBlendFactor::SourceAlpha);
        color_attachment
            .set_destination_rgb_blend_factor(metal::MTLBlendFactor::OneMinusSourceAlpha);
        color_attachment.set_source_alpha_blend_factor(metal::MTLBlendFactor::One);
        color_attachment
            .set_destination_alpha_blend_factor(metal::MTLBlendFactor::OneMinusSourceAlpha);

        match device.new_render_pipeline_state(&pipeline_desc) {
            Ok(state) => Some(state),
            Err(e) => {
                log::error!("Failed to create pipeline state: {}", e);
                None
            }
        }
    }

    /// Clear the vertex buffer for a new frame
    pub fn begin_frame(&mut self) {
        self.vertices.clear();
    }

    /// Add a line segment
    pub fn draw_line(
        &mut self,
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        color: [f32; 4],
        thickness: f32,
    ) {
        // Calculate perpendicular direction for thickness
        let dx = x2 - x1;
        let dy = y2 - y1;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 0.0001 {
            return;
        }

        let nx = -dy / len * thickness * 0.5;
        let ny = dx / len * thickness * 0.5;

        // Create quad from two triangles
        self.vertices.push(Vertex2D {
            position: [x1 - nx, y1 - ny],
            color,
        });
        self.vertices.push(Vertex2D {
            position: [x1 + nx, y1 + ny],
            color,
        });
        self.vertices.push(Vertex2D {
            position: [x2 + nx, y2 + ny],
            color,
        });

        self.vertices.push(Vertex2D {
            position: [x1 - nx, y1 - ny],
            color,
        });
        self.vertices.push(Vertex2D {
            position: [x2 + nx, y2 + ny],
            color,
        });
        self.vertices.push(Vertex2D {
            position: [x2 - nx, y2 - ny],
            color,
        });
    }

    /// Draw a polyline (connected line segments)
    pub fn draw_polyline(&mut self, points: &[[f32; 2]], color: [f32; 4], thickness: f32) {
        for i in 0..points.len().saturating_sub(1) {
            self.draw_line(
                points[i][0],
                points[i][1],
                points[i + 1][0],
                points[i + 1][1],
                color,
                thickness,
            );
        }
    }

    /// Draw a filled circle
    pub fn draw_circle(&mut self, cx: f32, cy: f32, radius: f32, color: [f32; 4], segments: u32) {
        let segments = segments.max(8);
        let angle_step = std::f32::consts::TAU / segments as f32;

        for i in 0..segments {
            let a1 = i as f32 * angle_step;
            let a2 = (i + 1) as f32 * angle_step;

            // Triangle fan from center
            self.vertices.push(Vertex2D {
                position: [cx, cy],
                color,
            });
            self.vertices.push(Vertex2D {
                position: [cx + radius * a1.cos(), cy + radius * a1.sin()],
                color,
            });
            self.vertices.push(Vertex2D {
                position: [cx + radius * a2.cos(), cy + radius * a2.sin()],
                color,
            });
        }
    }

    /// Draw a filled rectangle
    pub fn draw_rect(&mut self, x: f32, y: f32, width: f32, height: f32, color: [f32; 4]) {
        // Two triangles
        self.vertices.push(Vertex2D {
            position: [x, y],
            color,
        });
        self.vertices.push(Vertex2D {
            position: [x + width, y],
            color,
        });
        self.vertices.push(Vertex2D {
            position: [x + width, y + height],
            color,
        });

        self.vertices.push(Vertex2D {
            position: [x, y],
            color,
        });
        self.vertices.push(Vertex2D {
            position: [x + width, y + height],
            color,
        });
        self.vertices.push(Vertex2D {
            position: [x, y + height],
            color,
        });
    }

    /// Render the accumulated geometry
    ///
    /// # Safety
    /// `drawable` must be a valid CAMetalDrawable pointer
    pub unsafe fn render(&mut self, drawable: id, viewport_size: [f32; 2]) {
        if self.vertices.is_empty() {
            return;
        }

        // Ensure we don't exceed buffer capacity
        if self.vertices.len() > self.max_vertices {
            log::warn!(
                "Vertex count {} exceeds buffer capacity {}",
                self.vertices.len(),
                self.max_vertices
            );
            self.vertices.truncate(self.max_vertices);
        }

        // Copy vertices to GPU buffer
        let _data_size = self.vertices.len() * mem::size_of::<Vertex2D>();
        let buffer_ptr = self.vertex_buffer.contents() as *mut Vertex2D;
        unsafe {
            std::ptr::copy_nonoverlapping(self.vertices.as_ptr(), buffer_ptr, self.vertices.len());
        }

        // Get texture from drawable
        let texture: id = msg_send![drawable, texture];

        // Create render pass descriptor
        let render_pass_desc: id = msg_send![class!(MTLRenderPassDescriptor), renderPassDescriptor];
        let color_attachments: id = msg_send![render_pass_desc, colorAttachments];
        let color_attachment: id = msg_send![color_attachments, objectAtIndexedSubscript: 0_u64];

        let _: () = msg_send![color_attachment, setTexture: texture];
        let _: () = msg_send![color_attachment, setLoadAction: 1_u64]; // MTLLoadActionLoad (preserve background)
        let _: () = msg_send![color_attachment, setStoreAction: 1_u64]; // MTLStoreActionStore

        // Create command buffer and encoder using raw msg_send
        let command_queue_ptr = self.command_queue.as_ptr();
        let command_buffer: id = msg_send![command_queue_ptr, commandBuffer];
        let encoder: id =
            msg_send![command_buffer, renderCommandEncoderWithDescriptor: render_pass_desc];

        // Set pipeline state
        let _: () = msg_send![encoder, setRenderPipelineState: self.pipeline_state.as_ptr()];

        // Set vertex buffer
        let _: () = msg_send![encoder, setVertexBuffer: self.vertex_buffer.as_ptr() offset: 0_u64 atIndex: 0_u64];

        // Set viewport size uniform
        let viewport_buffer = self.device.new_buffer_with_data(
            viewport_size.as_ptr() as *const c_void,
            8,
            MTLResourceOptions::CPUCacheModeDefaultCache,
        );
        let _: () = msg_send![encoder, setVertexBuffer: viewport_buffer.as_ptr() offset: 0_u64 atIndex: 1_u64];

        // Draw triangles
        let _: () = msg_send![
            encoder,
            drawPrimitives: MTLPrimitiveType::Triangle
            vertexStart: 0_u64
            vertexCount: self.vertices.len() as u64
        ];

        // End encoding
        let _: () = msg_send![encoder, endEncoding];

        // Present and commit
        let _: () = msg_send![command_buffer, presentDrawable: drawable];
        let _: () = msg_send![command_buffer, commit];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vertex_size() {
        // Ensure vertex struct is properly packed
        assert_eq!(mem::size_of::<Vertex2D>(), 24); // 2*4 + 4*4 = 24 bytes
    }
}
