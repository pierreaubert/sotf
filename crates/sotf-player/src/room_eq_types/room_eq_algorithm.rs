use serde::{Deserialize, Serialize};

/// Optimization algorithm selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RoomEqAlgorithm {
    #[default]
    Cobyla,
    DifferentialEvolution,
    BayesianOptimization,
    CmaEs,
    /// Legacy value kept so older saved configs deserialize. It is not exposed
    /// in algorithm selectors now that `NLopt` backends are gone.
    NelderMead,
}

impl RoomEqAlgorithm {
    #[must_use]
    pub fn all() -> &'static [RoomEqAlgorithm] {
        &[
            RoomEqAlgorithm::Cobyla,
            RoomEqAlgorithm::DifferentialEvolution,
            RoomEqAlgorithm::BayesianOptimization,
            RoomEqAlgorithm::CmaEs,
        ]
    }

    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            RoomEqAlgorithm::Cobyla => "COBYLA",
            RoomEqAlgorithm::DifferentialEvolution => "Differential Evolution",
            RoomEqAlgorithm::BayesianOptimization => "Bayesian Optimization",
            RoomEqAlgorithm::CmaEs => "CMA-ES",
            RoomEqAlgorithm::NelderMead => "Nelder-Mead (Legacy)",
        }
    }

    #[must_use]
    pub fn to_autoeq_string(&self) -> &'static str {
        match self {
            RoomEqAlgorithm::DifferentialEvolution => "autoeq:de",
            RoomEqAlgorithm::BayesianOptimization => "autoeq:bo",
            RoomEqAlgorithm::CmaEs => "autoeq:cmaes",
            RoomEqAlgorithm::Cobyla | RoomEqAlgorithm::NelderMead => "autoeq:cobyla",
        }
    }
}
