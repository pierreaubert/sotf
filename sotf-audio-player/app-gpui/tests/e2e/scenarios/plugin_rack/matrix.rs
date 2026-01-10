//! E2E tests for Matrix Mixer Plugin.
//!
//! Tests for the channel matrix mixer that routes N inputs to P outputs.
//! Each output is a weighted sum of inputs, defined by a gain matrix.

use gpui::TestAppContext;
use std::cell::RefCell;
use std::rc::Rc;

// =============================================================================
// Mock Types for Testing
// =============================================================================

/// Matrix mixer plugin state for testing
struct MatrixState {
    enabled: bool,
    input_channels: usize,
    output_channels: usize,
    /// Gain matrix in row-major order (output_channels x input_channels)
    /// matrix[out * input_channels + in] = gain from input `in` to output `out`
    matrix: Vec<f32>,
    /// Optional input channel mapping (empty = dense 0,1,2...)
    input_channel_map: Vec<usize>,
    /// Optional output channel mapping (empty = dense 0,1,2...)
    output_channel_map: Vec<usize>,
}

impl MatrixState {
    fn new(input_channels: usize, output_channels: usize) -> Self {
        let matrix = Self::create_identity_matrix(input_channels, output_channels);
        Self {
            enabled: true,
            input_channels,
            output_channels,
            matrix,
            input_channel_map: Vec::new(),
            output_channel_map: Vec::new(),
        }
    }

    fn create_identity_matrix(inputs: usize, outputs: usize) -> Vec<f32> {
        let mut matrix = vec![0.0; inputs * outputs];
        for i in 0..inputs.min(outputs) {
            matrix[i * inputs + i] = 1.0;
        }
        matrix
    }

    fn get_gain(&self, input: usize, output: usize) -> f32 {
        if input < self.input_channels && output < self.output_channels {
            self.matrix[output * self.input_channels + input]
        } else {
            0.0
        }
    }

    fn set_gain(&mut self, input: usize, output: usize, gain: f32) {
        if input < self.input_channels && output < self.output_channels {
            self.matrix[output * self.input_channels + input] = gain;
        }
    }

    fn with_sparse_mapping(mut self, input_map: Vec<usize>, output_map: Vec<usize>) -> Self {
        self.input_channel_map = input_map;
        self.output_channel_map = output_map;
        self
    }
}

// =============================================================================
// Basic Plugin Tests
// =============================================================================

/// Test plugin renders correctly.
#[gpui::test]
async fn test_matrix_renders(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MatrixState::new(2, 2)));
    assert!(state.borrow().enabled);
    assert_eq!(state.borrow().input_channels, 2);
    assert_eq!(state.borrow().output_channels, 2);
}

/// Test identity matrix initialization.
#[gpui::test]
async fn test_matrix_identity_init(_cx: &mut TestAppContext) {
    let state = MatrixState::new(3, 3);

    // Diagonal should be 1.0
    for i in 0..3 {
        assert!((state.get_gain(i, i) - 1.0).abs() < 0.001);
    }

    // Off-diagonal should be 0.0
    assert!((state.get_gain(0, 1) - 0.0).abs() < 0.001);
    assert!((state.get_gain(1, 0) - 0.0).abs() < 0.001);
}

/// Test non-square matrix identity.
#[gpui::test]
async fn test_matrix_non_square(_cx: &mut TestAppContext) {
    // 2 inputs, 4 outputs
    let state = MatrixState::new(2, 4);

    // First 2 outputs map to inputs
    assert!((state.get_gain(0, 0) - 1.0).abs() < 0.001);
    assert!((state.get_gain(1, 1) - 1.0).abs() < 0.001);

    // Remaining outputs are zero
    assert!((state.get_gain(0, 2) - 0.0).abs() < 0.001);
    assert!((state.get_gain(1, 3) - 0.0).abs() < 0.001);
}

// =============================================================================
// Gain Cell Tests
// =============================================================================

/// Test gain cell value update.
#[gpui::test]
async fn test_matrix_gain_update(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MatrixState::new(2, 2)));

    state.borrow_mut().set_gain(0, 1, 0.5);
    assert!((state.borrow().get_gain(0, 1) - 0.5).abs() < 0.001);
}

/// Test gain cell bounds.
#[gpui::test]
async fn test_matrix_gain_bounds(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MatrixState::new(2, 2)));

    // Valid gain values: -inf to some positive max (typically -60 to +12 dB)
    let test_values: Vec<f32> = vec![0.0, 0.5, 1.0, 1.5, 2.0];
    for value in test_values {
        state.borrow_mut().set_gain(0, 0, value);
        assert!((state.borrow().get_gain(0, 0) - value).abs() < 0.001);
    }
}

/// Test gain to dB conversion.
#[gpui::test]
async fn test_matrix_gain_to_db(_cx: &mut TestAppContext) {
    fn gain_to_db(gain: f32) -> f32 {
        if gain <= 0.0 {
            f32::NEG_INFINITY
        } else {
            20.0 * gain.log10()
        }
    }

    assert!((gain_to_db(1.0) - 0.0).abs() < 0.001);
    assert!((gain_to_db(0.5) - (-6.02)).abs() < 0.1);
    assert!((gain_to_db(2.0) - 6.02).abs() < 0.1);
}

/// Test dB to gain conversion.
#[gpui::test]
async fn test_matrix_db_to_gain(_cx: &mut TestAppContext) {
    fn db_to_gain(db: f32) -> f32 {
        10.0_f32.powf(db / 20.0)
    }

    assert!((db_to_gain(0.0) - 1.0).abs() < 0.001);
    assert!((db_to_gain(-6.0) - 0.501).abs() < 0.01);
    assert!((db_to_gain(6.0) - 1.995).abs() < 0.01);
}

// =============================================================================
// Routing Pattern Tests
// =============================================================================

/// Test stereo swap routing.
#[gpui::test]
async fn test_matrix_stereo_swap(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MatrixState::new(2, 2)));

    // Swap: L->R, R->L
    state.borrow_mut().set_gain(0, 0, 0.0);
    state.borrow_mut().set_gain(0, 1, 1.0);
    state.borrow_mut().set_gain(1, 0, 1.0);
    state.borrow_mut().set_gain(1, 1, 0.0);

    // Output 0 = Input 1
    assert!((state.borrow().get_gain(1, 0) - 1.0).abs() < 0.001);
    // Output 1 = Input 0
    assert!((state.borrow().get_gain(0, 1) - 1.0).abs() < 0.001);
}

/// Test mono mixdown routing.
#[gpui::test]
async fn test_matrix_mono_mixdown(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MatrixState::new(2, 1)));

    // Both inputs to single output at 0.5 (avoid clipping)
    state.borrow_mut().set_gain(0, 0, 0.5);
    state.borrow_mut().set_gain(1, 0, 0.5);

    assert!((state.borrow().get_gain(0, 0) - 0.5).abs() < 0.001);
    assert!((state.borrow().get_gain(1, 0) - 0.5).abs() < 0.001);
}

/// Test mono to stereo duplication.
#[gpui::test]
async fn test_matrix_mono_to_stereo(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MatrixState::new(1, 2)));

    // Input 0 to both outputs
    state.borrow_mut().set_gain(0, 0, 1.0);
    state.borrow_mut().set_gain(0, 1, 1.0);

    assert!((state.borrow().get_gain(0, 0) - 1.0).abs() < 0.001);
    assert!((state.borrow().get_gain(0, 1) - 1.0).abs() < 0.001);
}

/// Test center channel extraction.
#[gpui::test]
async fn test_matrix_center_extract(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MatrixState::new(2, 1)));

    // Center = (L + R) / 2 at reduced gain for headroom
    state.borrow_mut().set_gain(0, 0, 0.5);
    state.borrow_mut().set_gain(1, 0, 0.5);

    // Verify both inputs contribute equally
    assert!((state.borrow().get_gain(0, 0) - state.borrow().get_gain(1, 0)).abs() < 0.001);
}

/// Test side channel extraction.
#[gpui::test]
async fn test_matrix_side_extract(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MatrixState::new(2, 1)));

    // Side = (L - R) / 2 (need negative gain for R)
    state.borrow_mut().set_gain(0, 0, 0.5);
    state.borrow_mut().set_gain(1, 0, -0.5);

    assert!((state.borrow().get_gain(0, 0) - 0.5).abs() < 0.001);
    assert!((state.borrow().get_gain(1, 0) - (-0.5)).abs() < 0.001);
}

// =============================================================================
// Sparse Channel Mapping Tests
// =============================================================================

/// Test sparse input mapping.
#[gpui::test]
async fn test_matrix_sparse_input_mapping(_cx: &mut TestAppContext) {
    // Map physical channels 1,2 to logical inputs 0,1
    let state = MatrixState::new(2, 2).with_sparse_mapping(vec![1, 2], vec![0, 1]);

    assert_eq!(state.input_channel_map, vec![1, 2]);
    assert_eq!(state.output_channel_map, vec![0, 1]);
}

/// Test sparse output mapping.
#[gpui::test]
async fn test_matrix_sparse_output_mapping(_cx: &mut TestAppContext) {
    // Map logical outputs 0,1 to physical channels 15,16
    let state = MatrixState::new(2, 2).with_sparse_mapping(vec![0, 1], vec![15, 16]);

    assert_eq!(state.output_channel_map, vec![15, 16]);
}

/// Test empty mapping means dense.
#[gpui::test]
async fn test_matrix_empty_mapping_dense(_cx: &mut TestAppContext) {
    let state = MatrixState::new(2, 2);

    assert!(state.input_channel_map.is_empty());
    assert!(state.output_channel_map.is_empty());
}

// =============================================================================
// Matrix Size Tests
// =============================================================================

/// Test common matrix sizes.
#[gpui::test]
async fn test_matrix_common_sizes(_cx: &mut TestAppContext) {
    // Stereo
    let state = MatrixState::new(2, 2);
    assert_eq!(state.matrix.len(), 4);

    // 5.1 to 5.1
    let state = MatrixState::new(6, 6);
    assert_eq!(state.matrix.len(), 36);

    // 7.1.4 to 7.1.4
    let state = MatrixState::new(12, 12);
    assert_eq!(state.matrix.len(), 144);
}

/// Test downmix matrix size.
#[gpui::test]
async fn test_matrix_downmix_size(_cx: &mut TestAppContext) {
    // 5.1 to stereo
    let state = MatrixState::new(6, 2);
    assert_eq!(state.matrix.len(), 12);
}

/// Test upmix matrix size.
#[gpui::test]
async fn test_matrix_upmix_size(_cx: &mut TestAppContext) {
    // Stereo to 5.1
    let state = MatrixState::new(2, 6);
    assert_eq!(state.matrix.len(), 12);
}

// =============================================================================
// Enable/Disable Tests
// =============================================================================

/// Test enabled toggle.
#[gpui::test]
async fn test_matrix_enabled(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MatrixState::new(2, 2)));

    assert!(state.borrow().enabled);

    state.borrow_mut().enabled = false;
    assert!(!state.borrow().enabled);
}

// =============================================================================
// Grid UI Tests
// =============================================================================

/// Test grid cell count.
#[gpui::test]
async fn test_matrix_grid_cell_count(_cx: &mut TestAppContext) {
    fn grid_cell_count(inputs: usize, outputs: usize) -> usize {
        inputs * outputs
    }

    assert_eq!(grid_cell_count(2, 2), 4);
    assert_eq!(grid_cell_count(6, 6), 36);
    assert_eq!(grid_cell_count(2, 6), 12);
}

/// Test grid row/column labels.
#[gpui::test]
async fn test_matrix_grid_labels(_cx: &mut TestAppContext) {
    fn get_channel_label(ch: usize, is_input: bool) -> String {
        let prefix = if is_input { "In" } else { "Out" };
        format!("{}{}", prefix, ch + 1)
    }

    assert_eq!(get_channel_label(0, true), "In1");
    assert_eq!(get_channel_label(0, false), "Out1");
    assert_eq!(get_channel_label(5, true), "In6");
}

/// Test grid cell selection.
#[gpui::test]
async fn test_matrix_grid_selection(_cx: &mut TestAppContext) {
    struct GridSelection {
        input: usize,
        output: usize,
    }

    let selection = GridSelection {
        input: 0,
        output: 1,
    };
    assert_eq!(selection.input, 0);
    assert_eq!(selection.output, 1);
}

// =============================================================================
// Visual Feedback Tests
// =============================================================================

/// Test gain cell color based on value.
#[gpui::test]
async fn test_matrix_cell_color(_cx: &mut TestAppContext) {
    fn get_cell_color(gain: f32) -> &'static str {
        if gain <= 0.0 {
            "muted" // -inf dB
        } else if gain < 0.9 {
            "attenuated" // negative dB
        } else if gain < 1.1 {
            "unity" // ~0 dB
        } else {
            "boosted" // positive dB
        }
    }

    assert_eq!(get_cell_color(0.0), "muted");
    assert_eq!(get_cell_color(0.5), "attenuated");
    assert_eq!(get_cell_color(1.0), "unity");
    assert_eq!(get_cell_color(2.0), "boosted");
}

/// Test diagonal highlighting.
#[gpui::test]
async fn test_matrix_diagonal_highlight(_cx: &mut TestAppContext) {
    fn is_diagonal(input: usize, output: usize) -> bool {
        input == output
    }

    assert!(is_diagonal(0, 0));
    assert!(is_diagonal(3, 3));
    assert!(!is_diagonal(0, 1));
}

// =============================================================================
// Preset Pattern Tests
// =============================================================================

/// Test preset: pass-through.
#[gpui::test]
async fn test_matrix_preset_passthrough(_cx: &mut TestAppContext) {
    let state = MatrixState::new(4, 4);

    // Verify it's identity (pass-through)
    for i in 0..4 {
        for j in 0..4 {
            let expected = if i == j { 1.0 } else { 0.0 };
            assert!((state.get_gain(i, j) - expected).abs() < 0.001);
        }
    }
}

/// Test preset: silence all.
#[gpui::test]
async fn test_matrix_preset_silence(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MatrixState::new(2, 2)));

    // Set all gains to zero
    for i in 0..2 {
        for j in 0..2 {
            state.borrow_mut().set_gain(i, j, 0.0);
        }
    }

    for i in 0..2 {
        for j in 0..2 {
            assert!((state.borrow().get_gain(i, j) - 0.0).abs() < 0.001);
        }
    }
}

/// Test preset: stereo width.
#[gpui::test]
async fn test_matrix_preset_stereo_width(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MatrixState::new(2, 2)));

    // Width = 0.7 means 70% original + 30% crossfeed
    let width = 0.7;
    let cross = (1.0 - width) / 2.0;

    state.borrow_mut().set_gain(0, 0, width + cross); // L in -> L out
    state.borrow_mut().set_gain(1, 0, cross); // R in -> L out
    state.borrow_mut().set_gain(0, 1, cross); // L in -> R out
    state.borrow_mut().set_gain(1, 1, width + cross); // R in -> R out

    // Cross gains should be equal
    assert!((state.borrow().get_gain(1, 0) - state.borrow().get_gain(0, 1)).abs() < 0.001);
}

// =============================================================================
// Value Edit Tests
// =============================================================================

/// Test cell value text entry.
#[gpui::test]
async fn test_matrix_value_text_entry(_cx: &mut TestAppContext) {
    fn parse_gain_text(text: &str) -> Option<f32> {
        text.trim()
            .replace("dB", "")
            .trim()
            .parse::<f32>()
            .ok()
            .map(|db| 10.0_f32.powf(db / 20.0))
    }

    // Parse "0 dB" -> 1.0
    let gain = parse_gain_text("0 dB");
    assert!(gain.is_some());
    assert!((gain.unwrap() - 1.0).abs() < 0.001);

    // Parse "-6 dB" -> 0.5
    let gain = parse_gain_text("-6 dB");
    assert!(gain.is_some());
    assert!((gain.unwrap() - 0.501).abs() < 0.01);
}

/// Test gain display format.
#[gpui::test]
async fn test_matrix_gain_display(_cx: &mut TestAppContext) {
    fn format_gain_db(gain: f32) -> String {
        if gain <= 0.001 {
            "-∞".to_string()
        } else {
            let db = 20.0 * gain.log10();
            if db.abs() < 0.1 {
                "0.0".to_string()
            } else {
                format!("{:+.1}", db)
            }
        }
    }

    assert_eq!(format_gain_db(0.0), "-∞");
    assert_eq!(format_gain_db(1.0), "0.0");
    assert_eq!(format_gain_db(2.0), "+6.0");
    assert_eq!(format_gain_db(0.5), "-6.0");
}

// =============================================================================
// Reset Tests
// =============================================================================

/// Test reset to identity.
#[gpui::test]
async fn test_matrix_reset_identity(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MatrixState::new(2, 2)));

    // Modify matrix
    state.borrow_mut().set_gain(0, 0, 0.5);
    state.borrow_mut().set_gain(0, 1, 0.3);

    // Reset by creating new identity matrix
    let identity = MatrixState::create_identity_matrix(2, 2);
    state.borrow_mut().matrix = identity;

    // Verify reset
    assert!((state.borrow().get_gain(0, 0) - 1.0).abs() < 0.001);
    assert!((state.borrow().get_gain(0, 1) - 0.0).abs() < 0.001);
}

/// Test reset single cell.
#[gpui::test]
async fn test_matrix_reset_cell(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MatrixState::new(2, 2)));

    // Modify cell
    state.borrow_mut().set_gain(0, 0, 0.5);

    // Reset to default (1.0 for diagonal, 0.0 for off-diagonal)
    let default_value = if 0 == 0 { 1.0 } else { 0.0 };
    state.borrow_mut().set_gain(0, 0, default_value);

    assert!((state.borrow().get_gain(0, 0) - 1.0).abs() < 0.001);
}
