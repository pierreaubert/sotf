use super::misc::is_linear_phase_type;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CrossoverKind {
    Lr24,
    LinearPhase,
}

impl CrossoverKind {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Lr24 => "LR24",
            Self::LinearPhase => "LinearPhase",
        }
    }

    pub(super) fn parse(crossover_type: &str) -> Result<Self, String> {
        if crossover_type.eq_ignore_ascii_case("lr24") || crossover_type.eq_ignore_ascii_case("lr4")
        {
            Ok(Self::Lr24)
        } else if is_linear_phase_type(crossover_type) {
            Ok(Self::LinearPhase)
        } else {
            Err(format!(
                "Unsupported crossover type: '{}'. Supported: LR24/LR4 and LinearPhase/FIR.",
                crossover_type
            ))
        }
    }
}
