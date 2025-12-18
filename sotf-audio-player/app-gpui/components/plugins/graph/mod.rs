//! Plugin Graph UI Components
//!
//! Provides a 2D canvas-based interface for managing plugins as nodes
//! with channel-level connections.

mod cable;
mod canvas;
mod node;

pub use cable::CableElement;
pub use canvas::GraphCanvas;
pub use node::{GraphNode, PortType};
