/// Per-note expression kind timestamped relative to a processing block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteExpressionKind {
    PitchBend,
    Pressure,
    Timbre,
    Brightness,
    Volume,
    Pan,
}

/// Result type for plugin operations
pub type PluginResult<T> = Result<T, String>;

/// Coarse processing-cost category used by hosts for scheduling decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginCostClass {
    /// Very cheap scalar/sample operations such as gain, mute, or routing.
    Scalar,
    /// Stateful IIR/filter-bank style processing.
    Iir,
    /// Envelope, dynamics, or nonlinear per-sample processing.
    Dynamics,
    /// FFT/STFT or spectral block processing.
    Fft,
    /// Convolution/FIR partition processing.
    Convolution,
    /// Analyzer plugins that usually observe rather than transform.
    Analyzer,
    /// Out-of-process or third-party plugin processing.
    External,
}

/// Specialized operation selected by a host compiled render plan.
///
/// Plugins may choose to implement these directly to bypass generic adapter
/// dispatch while preserving their own state and DSP invariants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginCompiledOp {
    /// Scalar gain multiplication.
    ApplyGain,
    /// Stateful EQ/filter-bank processing.
    EqBiquadBank,
    /// Per-channel mute/solo/dim processing.
    ChannelMuteSolo,
    /// Brick-wall limiter dynamics processing.
    Limiter,
    /// Multiband compressor dynamics processing.
    MultibandCompressor,
    /// Analyzer pass-through with side-channel state updates.
    AnalyzerTap,
}

/// Parameter-sensitive compile/fusion contract for the plugin's current state.
///
/// This is intentionally conservative. Plugins should only advertise properties
/// that are true for the current parameter/state combination; for example, a
/// limiter with lookahead reports latency and will not be fused across.
#[derive(Debug, Clone, PartialEq)]
pub struct PluginCompileMetadata {
    /// Coarse cost class for scheduling and initial planning.
    pub cost_class: PluginCostClass,
    /// Preferred compiled operation, if one is currently legal.
    pub compiled_op: Option<PluginCompiledOp>,
    /// Stable scalar gain that can be folded into adjacent compatible ops.
    pub static_gain: Option<f32>,
    /// Whether the processing is linear for the current parameter state.
    pub linear: bool,
    /// Whether coefficients/transfer behavior are stable for the current block.
    pub time_invariant_for_block: bool,
    /// Whether channels are mixed together.
    pub channel_mixing: bool,
    /// Whether the plugin has audio-rate state that must be advanced in order.
    pub stateful: bool,
    /// Processing latency in samples for the current state.
    pub latency_samples: usize,
    /// Whether a static global input gain can legally move through this op.
    pub can_absorb_input_gain: bool,
    /// Whether a static global output gain can legally move through this op.
    pub can_absorb_output_gain: bool,
    /// Whether this op can merge with adjacent compatible EQ/filter sections.
    pub can_merge_with_eq: bool,
    /// Whether this op must terminate a fused region.
    pub boundary: bool,
}

impl PluginCompileMetadata {
    pub fn boundary(cost_class: PluginCostClass, latency_samples: usize) -> Self {
        Self {
            cost_class,
            compiled_op: None,
            static_gain: None,
            linear: false,
            time_invariant_for_block: false,
            channel_mixing: false,
            stateful: true,
            latency_samples,
            can_absorb_input_gain: false,
            can_absorb_output_gain: false,
            can_merge_with_eq: false,
            boundary: true,
        }
    }

    pub fn linear_transform(
        cost_class: PluginCostClass,
        compiled_op: Option<PluginCompiledOp>,
        latency_samples: usize,
        channel_mixing: bool,
        stateful: bool,
        can_merge_with_eq: bool,
    ) -> Self {
        Self {
            cost_class,
            compiled_op,
            static_gain: None,
            linear: true,
            time_invariant_for_block: true,
            channel_mixing,
            stateful,
            latency_samples,
            can_absorb_input_gain: true,
            can_absorb_output_gain: true,
            can_merge_with_eq,
            boundary: latency_samples > 0,
        }
    }

    pub fn nonlinear(
        cost_class: PluginCostClass,
        compiled_op: Option<PluginCompiledOp>,
        latency_samples: usize,
        channel_mixing: bool,
    ) -> Self {
        Self {
            cost_class,
            compiled_op,
            static_gain: None,
            linear: false,
            time_invariant_for_block: false,
            channel_mixing,
            stateful: true,
            latency_samples,
            can_absorb_input_gain: false,
            can_absorb_output_gain: false,
            can_merge_with_eq: false,
            boundary: true,
        }
    }

    pub fn routing(
        cost_class: PluginCostClass,
        compiled_op: Option<PluginCompiledOp>,
        channel_mixing: bool,
    ) -> Self {
        Self {
            cost_class,
            compiled_op,
            static_gain: None,
            linear: true,
            time_invariant_for_block: true,
            channel_mixing,
            stateful: false,
            latency_samples: 0,
            can_absorb_input_gain: true,
            can_absorb_output_gain: true,
            can_merge_with_eq: false,
            boundary: false,
        }
    }

    pub fn analyzer(compiled_op: Option<PluginCompiledOp>) -> Self {
        Self {
            cost_class: PluginCostClass::Analyzer,
            compiled_op,
            static_gain: None,
            linear: true,
            time_invariant_for_block: false,
            channel_mixing: false,
            stateful: true,
            latency_samples: 0,
            can_absorb_input_gain: false,
            can_absorb_output_gain: false,
            can_merge_with_eq: false,
            boundary: true,
        }
    }
}
