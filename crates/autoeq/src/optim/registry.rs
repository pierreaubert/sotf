//! Algorithm registry — maps user-facing algorithm names to backend impls.
//!
//! Replaces the old `get_all_algorithms()` / `find_algorithm_info()` /
//! `parse_algorithm_name()` triple in [`super`]. Names accept the
//! library-prefixed canonical form (`"autoeq:cobyla"`, `"mh:de"`) and the
//! legacy unprefixed alias (`"cobyla"`, `"isres"`) — the alias resolves
//! to the *first* matching algorithm in [`all_algorithms`], which is now
//! the pure-Rust path.
//!
//! ## NLopt deprecation
//!
//! The C-FFI nlopt backend has been removed. The `nlopt:*` prefix is no
//! longer registered. For user configs that still pin a removed name
//! (e.g. roomeq JSON presets shipped before the migration),
//! [`resolve`] applies a fallback table: any `nlopt:*` name maps to the
//! closest pure-Rust equivalent and a one-shot warning is logged.

use super::backend::FilterOptimizer;
use std::sync::OnceLock;

/// All registered algorithms, in the order they appear in `--algo-list`.
pub fn all_algorithms() -> Vec<Box<dyn FilterOptimizer>> {
    let mut algos: Vec<Box<dyn FilterOptimizer>> = Vec::new();

    // Pure-Rust backends from math-optimisation. Order matters: legacy
    // unprefixed names ("cobyla", "isres") resolve via suffix-match to
    // the *first* entry, so the autoeq:* variants are listed first.
    use super::cobyla::AutoeqCobylaBackend;
    algos.push(Box::new(AutoeqCobylaBackend::new("autoeq:cobyla")));

    use super::isres::AutoeqIsresBackend;
    algos.push(Box::new(AutoeqIsresBackend::new("autoeq:isres")));

    use super::cmaes::AutoeqCmaEsBackend;
    algos.push(Box::new(AutoeqCmaEsBackend::new("autoeq:cmaes")));

    use super::bo::AutoeqBoBackend;
    algos.push(Box::new(AutoeqBoBackend::new("autoeq:bo")));

    use super::nsga::AutoeqNsgaBackend;
    algos.push(Box::new(AutoeqNsgaBackend::new_nsga2("autoeq:nsga2")));
    algos.push(Box::new(AutoeqNsgaBackend::new_nsga3("autoeq:nsga3")));

    use super::de::AutoeqDeBackend;
    algos.push(Box::new(AutoeqDeBackend::new("autoeq:de")));

    use super::mh::MhBackend;
    algos.push(Box::new(MhBackend::new_de("mh:de")));
    algos.push(Box::new(MhBackend::new_pso("mh:pso")));
    algos.push(Box::new(MhBackend::new_rga("mh:rga")));
    algos.push(Box::new(MhBackend::new_tlbo("mh:tlbo")));
    algos.push(Box::new(MhBackend::new_firefly("mh:firefly")));

    algos
}

/// Map a removed `nlopt:*` name to the closest still-registered pure-Rust
/// algorithm. Returns `None` if the input isn't a recognised nlopt alias.
fn nlopt_deprecation_map(name: &str) -> Option<&'static str> {
    let lower = name.to_lowercase();
    let mapped = match lower.as_str() {
        // Local optimizers with nonlinear constraint support → autoeq:cobyla
        "nlopt:cobyla" | "nlopt:slsqp" => "autoeq:cobyla",
        // Local optimizers without constraint support → still cobyla (best
        // local option in the new registry).
        "nlopt:bobyqa" | "nlopt:neldermead" | "nlopt:sbplx" => "autoeq:cobyla",
        // Global optimizers with nonlinear constraint support → autoeq:isres
        "nlopt:isres" | "nlopt:ags" | "nlopt:origdirect" => "autoeq:isres",
        // Global optimizers without constraint support → autoeq:de (better
        // than ISRES on these unconstrained-style problems and our default).
        "nlopt:crs2lm" | "nlopt:direct" | "nlopt:directl" | "nlopt:gmlsl" | "nlopt:gmlsllds"
        | "nlopt:stogo" | "nlopt:stogorand" => "autoeq:de",
        _ => return None,
    };
    Some(mapped)
}

/// Resolve `name` to a backend.
///
/// Resolution order:
/// 1. Exact match on canonical prefixed name (`"autoeq:cobyla"`).
/// 2. Suffix match for legacy unprefixed names (`"cobyla"` → first entry
///    whose suffix is `"cobyla"`, which is `"autoeq:cobyla"`).
/// 3. Removed-NLopt fallback: `"nlopt:cobyla"` → `"autoeq:cobyla"` etc.,
///    with a one-shot deprecation warning.
pub fn resolve(name: &str) -> Option<Box<dyn FilterOptimizer>> {
    let mut algos = all_algorithms();

    if let Some(idx) = algos
        .iter()
        .position(|a| a.name().eq_ignore_ascii_case(name))
    {
        return Some(algos.swap_remove(idx));
    }

    let name_lower = name.to_lowercase();
    if let Some(idx) = algos.iter().position(|a| {
        a.name()
            .split(':')
            .nth(1)
            .map(|s| s.eq_ignore_ascii_case(&name_lower))
            .unwrap_or(false)
    }) {
        return Some(algos.swap_remove(idx));
    }

    if let Some(alias) = canonical_alias(name) {
        return resolve(alias);
    }

    if let Some(replacement) = nlopt_deprecation_map(name) {
        warn_deprecated_once(name, replacement);
        // Re-enter resolve once with the replacement name. Bounded
        // recursion: replacement is always a registered prefixed name.
        return resolve(replacement);
    }

    None
}

fn canonical_alias(name: &str) -> Option<&'static str> {
    let lower = name.to_lowercase();
    match lower.as_str() {
        "cma-es" | "cma_es" | "autoeq:cma-es" | "autoeq:cma_es" => Some("autoeq:cmaes"),
        "bayes" | "bayesian" | "bayesopt" | "bo-gp" | "autoeq:bayes" | "autoeq:bayesian" => {
            Some("autoeq:bo")
        }
        "nsga-ii" | "nsga_ii" | "autoeq:nsga-ii" | "autoeq:nsga_ii" => Some("autoeq:nsga2"),
        "nsga-iii" | "nsga_iii" | "autoeq:nsga-iii" | "autoeq:nsga_iii" => Some("autoeq:nsga3"),
        _ => None,
    }
}

/// One-shot deprecation logger keyed by the removed name. Prevents log
/// spam when the same algorithm appears in many roomeq configs.
fn warn_deprecated_once(removed: &str, replacement: &str) {
    use std::sync::Mutex;
    static SEEN: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
    let seen = SEEN.get_or_init(|| Mutex::new(Vec::new()));
    let mut seen = seen.lock().unwrap();
    let key = removed.to_lowercase();
    if !seen.iter().any(|k| k == &key) {
        seen.push(key);
        log::warn!(
            "Algorithm '{}' was removed when the C-FFI nlopt dependency was dropped. \
             Falling back to '{}' (pure-Rust). Update your config to silence this warning.",
            removed,
            replacement,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_bayesian_optimizer_aliases() {
        for name in ["autoeq:bo", "bo", "bayes", "bayesian", "autoeq:bayes"] {
            let backend = resolve(name).expect("BO backend should resolve");
            assert_eq!(backend.name(), "autoeq:bo");
        }
    }
}
