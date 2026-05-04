//! NSGA-II and NSGA-III Pareto multi-objective optimizers.
//!
//! The implementation minimises every objective. It uses Deb's fast
//! non-dominated sorting, simulated binary crossover, polynomial mutation,
//! crowding-distance survival for NSGA-II, and reference-direction niching for
//! NSGA-III.

use ndarray::Array1;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::error::{DEError, Result};

/// NSGA environmental-selection variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NsgaVariant {
    /// NSGA-II: crowding-distance sorting inside the splitting front.
    Nsga2,
    /// NSGA-III: reference-direction niching inside the splitting front.
    Nsga3,
}

/// Configuration for [`nsga`].
#[derive(Debug, Clone)]
pub struct NsgaConfig {
    /// `(lower, upper)` bounds per parameter.
    pub bounds: Vec<(f64, f64)>,
    /// Optional initial individual. Values outside bounds are clipped.
    pub x0: Option<Array1<f64>>,
    /// Population size. `0` uses `max(40, 10 * n_params)`.
    pub population_size: usize,
    /// Maximum objective evaluations.
    pub maxeval: usize,
    /// Simulated-binary crossover probability.
    pub crossover_prob: f64,
    /// Per-variable polynomial mutation probability. `None` uses `1 / n`.
    pub mutation_prob: Option<f64>,
    /// Simulated-binary crossover distribution index.
    pub eta_c: f64,
    /// Polynomial mutation distribution index.
    pub eta_m: f64,
    /// NSGA-II or NSGA-III survival selection.
    pub variant: NsgaVariant,
    /// Das-Dennis reference-direction partitions for NSGA-III.
    ///
    /// `0` selects a dimension-dependent default.
    pub reference_partitions: usize,
    /// Optional RNG seed for deterministic runs.
    pub seed: Option<u64>,
}

impl Default for NsgaConfig {
    fn default() -> Self {
        Self {
            bounds: Vec::new(),
            x0: None,
            population_size: 0,
            maxeval: 10_000,
            crossover_prob: 0.9,
            mutation_prob: None,
            eta_c: 15.0,
            eta_m: 20.0,
            variant: NsgaVariant::Nsga2,
            reference_partitions: 0,
            seed: None,
        }
    }
}

/// A Pareto solution returned by [`NsgaReport`].
#[derive(Debug, Clone)]
pub struct ParetoSolution {
    /// Parameter vector.
    pub x: Array1<f64>,
    /// Objective values, all minimised.
    pub objectives: Vec<f64>,
    /// Non-dominated rank. Rank `0` is the Pareto front.
    pub rank: usize,
    /// Crowding distance used by NSGA-II. Boundary points use infinity.
    pub crowding_distance: f64,
}

/// Result of an [`nsga`] run.
#[derive(Debug, Clone)]
pub struct NsgaReport {
    /// Rank-0 non-dominated front from the final population.
    pub pareto_front: Vec<ParetoSolution>,
    /// Final population, ranked and crowding-scored.
    pub population: Vec<ParetoSolution>,
    /// Objective evaluations consumed.
    pub nfev: usize,
    /// Generations completed.
    pub nit: usize,
    /// Whether at least one generation completed.
    pub success: bool,
    /// Human-readable termination message.
    pub message: String,
}

#[derive(Clone)]
struct Individual {
    x: Array1<f64>,
    objectives: Vec<f64>,
    rank: usize,
    crowding_distance: f64,
}

/// Run NSGA-II with crowding-distance survival selection.
pub fn nsga2<F>(f: &F, mut config: NsgaConfig) -> Result<NsgaReport>
where
    F: Fn(&Array1<f64>) -> Vec<f64> + Sync,
{
    config.variant = NsgaVariant::Nsga2;
    nsga(f, config)
}

/// Run NSGA-III with reference-direction niching survival selection.
pub fn nsga3<F>(f: &F, mut config: NsgaConfig) -> Result<NsgaReport>
where
    F: Fn(&Array1<f64>) -> Vec<f64> + Sync,
{
    config.variant = NsgaVariant::Nsga3;
    nsga(f, config)
}

/// Run NSGA-II or NSGA-III according to [`NsgaConfig::variant`].
pub fn nsga<F>(f: &F, config: NsgaConfig) -> Result<NsgaReport>
where
    F: Fn(&Array1<f64>) -> Vec<f64> + Sync,
{
    validate_config(&config)?;
    let n = config.bounds.len();
    let pop_size = if config.population_size == 0 {
        (10 * n).max(40)
    } else {
        config.population_size
    }
    .max(4);
    let mutation_prob = config
        .mutation_prob
        .unwrap_or(1.0 / n as f64)
        .clamp(0.0, 1.0);

    let mut rng: StdRng = match config.seed {
        Some(s) => StdRng::seed_from_u64(s),
        None => {
            let mut thread_rng = rand::rng();
            StdRng::from_rng(&mut thread_rng)
        }
    };

    let mut population = initial_population(f, &config, pop_size, &mut rng);
    let mut nfev = population.len();
    assign_rank_and_crowding(&mut population);

    let mut nit = 0usize;
    while nfev < config.maxeval {
        let mut offspring: Vec<Individual> = Vec::with_capacity(pop_size);
        while offspring.len() < pop_size && nfev < config.maxeval {
            let p1 = tournament_select(&population, &mut rng);
            let p2 = tournament_select(&population, &mut rng);
            let (mut c1, mut c2) = if rng.random::<f64>() < config.crossover_prob {
                sbx_crossover(
                    &p1.x,
                    &p2.x,
                    &config.bounds,
                    config.eta_c.max(1.0),
                    &mut rng,
                )
            } else {
                (p1.x.clone(), p2.x.clone())
            };
            polynomial_mutation(
                &mut c1,
                &config.bounds,
                mutation_prob,
                config.eta_m.max(1.0),
                &mut rng,
            );
            polynomial_mutation(
                &mut c2,
                &config.bounds,
                mutation_prob,
                config.eta_m.max(1.0),
                &mut rng,
            );

            offspring.push(evaluate(f, c1));
            nfev += 1;
            if offspring.len() < pop_size && nfev < config.maxeval {
                offspring.push(evaluate(f, c2));
                nfev += 1;
            }
        }

        if offspring.is_empty() {
            break;
        }
        population.extend(offspring);
        population = environmental_selection(population, pop_size, &config);
        nit += 1;
    }

    assign_rank_and_crowding(&mut population);
    let mut final_solutions = to_solutions(&population);
    final_solutions.sort_by(compare_solutions);
    let pareto_front = final_solutions
        .iter()
        .filter(|s| s.rank == 0)
        .cloned()
        .collect::<Vec<_>>();

    Ok(NsgaReport {
        pareto_front,
        population: final_solutions,
        nfev,
        nit,
        success: nit > 0,
        message: if nit > 0 {
            String::from("evaluation budget reached")
        } else {
            String::from("initial population evaluated")
        },
    })
}

fn validate_config(config: &NsgaConfig) -> Result<()> {
    let n = config.bounds.len();
    if n == 0 {
        return Err(DEError::BoundsMismatch {
            lower_len: 0,
            upper_len: 0,
        });
    }
    for (i, (lo, hi)) in config.bounds.iter().enumerate() {
        if lo > hi {
            return Err(DEError::InvalidBounds {
                index: i,
                lower: *lo,
                upper: *hi,
            });
        }
    }
    if let Some(ref x0) = config.x0
        && x0.len() != n
    {
        return Err(DEError::X0DimensionMismatch {
            expected: n,
            got: x0.len(),
        });
    }
    Ok(())
}

fn initial_population<F>(
    f: &F,
    config: &NsgaConfig,
    pop_size: usize,
    rng: &mut StdRng,
) -> Vec<Individual>
where
    F: Fn(&Array1<f64>) -> Vec<f64> + Sync,
{
    let mut population = Vec::with_capacity(pop_size);
    if let Some(ref x0) = config.x0 {
        let x = clamp_to_bounds(x0.clone(), &config.bounds);
        population.push(evaluate(f, x));
    }
    while population.len() < pop_size {
        let x = random_in_bounds(&config.bounds, rng);
        population.push(evaluate(f, x));
    }
    population
}

fn evaluate<F>(f: &F, x: Array1<f64>) -> Individual
where
    F: Fn(&Array1<f64>) -> Vec<f64> + Sync,
{
    let objectives = f(&x)
        .into_iter()
        .map(|v| if v.is_finite() { v } else { f64::INFINITY })
        .collect();
    Individual {
        x,
        objectives,
        rank: usize::MAX,
        crowding_distance: 0.0,
    }
}

fn random_in_bounds(bounds: &[(f64, f64)], rng: &mut StdRng) -> Array1<f64> {
    Array1::from(
        bounds
            .iter()
            .map(|(lo, hi)| {
                if hi > lo {
                    rng.random_range(*lo..=*hi)
                } else {
                    *lo
                }
            })
            .collect::<Vec<_>>(),
    )
}

fn clamp_to_bounds(mut x: Array1<f64>, bounds: &[(f64, f64)]) -> Array1<f64> {
    for (xi, (lo, hi)) in x.iter_mut().zip(bounds.iter()) {
        *xi = xi.clamp(*lo, *hi);
    }
    x
}

fn tournament_select<'a>(population: &'a [Individual], rng: &mut StdRng) -> &'a Individual {
    let a = rng.random_range(0..population.len());
    let b = rng.random_range(0..population.len());
    let ia = &population[a];
    let ib = &population[b];
    if ia.rank < ib.rank || (ia.rank == ib.rank && ia.crowding_distance > ib.crowding_distance) {
        ia
    } else {
        ib
    }
}

fn sbx_crossover(
    p1: &Array1<f64>,
    p2: &Array1<f64>,
    bounds: &[(f64, f64)],
    eta_c: f64,
    rng: &mut StdRng,
) -> (Array1<f64>, Array1<f64>) {
    let mut c1 = p1.clone();
    let mut c2 = p2.clone();
    for i in 0..p1.len() {
        if rng.random::<f64>() > 0.5 || (p1[i] - p2[i]).abs() <= 1e-14 {
            continue;
        }
        let u = rng
            .random::<f64>()
            .clamp(f64::MIN_POSITIVE, 1.0 - f64::EPSILON);
        let beta = if u <= 0.5 {
            (2.0 * u).powf(1.0 / (eta_c + 1.0))
        } else {
            (1.0 / (2.0 * (1.0 - u))).powf(1.0 / (eta_c + 1.0))
        };
        c1[i] = 0.5 * ((1.0 + beta) * p1[i] + (1.0 - beta) * p2[i]);
        c2[i] = 0.5 * ((1.0 - beta) * p1[i] + (1.0 + beta) * p2[i]);
        c1[i] = c1[i].clamp(bounds[i].0, bounds[i].1);
        c2[i] = c2[i].clamp(bounds[i].0, bounds[i].1);
    }
    (c1, c2)
}

fn polynomial_mutation(
    x: &mut Array1<f64>,
    bounds: &[(f64, f64)],
    mutation_prob: f64,
    eta_m: f64,
    rng: &mut StdRng,
) {
    for i in 0..x.len() {
        if rng.random::<f64>() > mutation_prob {
            continue;
        }
        let (lo, hi) = bounds[i];
        let span = hi - lo;
        if span <= 0.0 {
            x[i] = lo;
            continue;
        }
        let y = x[i].clamp(lo, hi);
        let delta1 = (y - lo) / span;
        let delta2 = (hi - y) / span;
        let rnd = rng.random::<f64>();
        let mut_pow = 1.0 / (eta_m + 1.0);
        let deltaq = if rnd <= 0.5 {
            let xy = 1.0 - delta1;
            let val = 2.0 * rnd + (1.0 - 2.0 * rnd) * xy.powf(eta_m + 1.0);
            val.powf(mut_pow) - 1.0
        } else {
            let xy = 1.0 - delta2;
            let val = 2.0 * (1.0 - rnd) + 2.0 * (rnd - 0.5) * xy.powf(eta_m + 1.0);
            1.0 - val.powf(mut_pow)
        };
        x[i] = (y + deltaq * span).clamp(lo, hi);
    }
}

fn environmental_selection(
    mut combined: Vec<Individual>,
    pop_size: usize,
    config: &NsgaConfig,
) -> Vec<Individual> {
    let fronts = assign_rank_and_crowding(&mut combined);
    let mut next = Vec::with_capacity(pop_size);

    for front in fronts {
        if next.len() + front.len() <= pop_size {
            next.extend(front.into_iter().map(|idx| combined[idx].clone()));
            continue;
        }

        let remaining = pop_size - next.len();
        let splitting_front = front
            .into_iter()
            .map(|idx| combined[idx].clone())
            .collect::<Vec<_>>();
        match config.variant {
            NsgaVariant::Nsga2 => {
                let mut partial = splitting_front;
                let partial_indices = (0..partial.len()).collect::<Vec<_>>();
                assign_crowding_distance_for_indices(&mut partial, &partial_indices);
                partial.sort_by(compare_individuals);
                next.extend(partial.into_iter().take(remaining));
            }
            NsgaVariant::Nsga3 => {
                let chosen = nsga3_niching(&next, splitting_front, remaining, config);
                next.extend(chosen);
            }
        }
        break;
    }

    assign_rank_and_crowding(&mut next);
    next
}

fn assign_rank_and_crowding(population: &mut [Individual]) -> Vec<Vec<usize>> {
    let fronts = non_dominated_sort(population);
    for front in &fronts {
        assign_crowding_distance_for_indices(population, front);
    }
    fronts
}

fn non_dominated_sort(population: &mut [Individual]) -> Vec<Vec<usize>> {
    let n = population.len();
    let mut dominates_set = vec![Vec::<usize>::new(); n];
    let mut domination_count = vec![0usize; n];
    let mut fronts: Vec<Vec<usize>> = Vec::new();
    let mut first = Vec::new();

    for p in 0..n {
        for q in 0..n {
            if p == q {
                continue;
            }
            if dominates(&population[p].objectives, &population[q].objectives) {
                dominates_set[p].push(q);
            } else if dominates(&population[q].objectives, &population[p].objectives) {
                domination_count[p] += 1;
            }
        }
        if domination_count[p] == 0 {
            population[p].rank = 0;
            first.push(p);
        }
    }
    fronts.push(first);

    let mut i = 0usize;
    while i < fronts.len() {
        let mut next = Vec::new();
        for &p in &fronts[i] {
            for &q in &dominates_set[p] {
                domination_count[q] = domination_count[q].saturating_sub(1);
                if domination_count[q] == 0 {
                    population[q].rank = i + 1;
                    next.push(q);
                }
            }
        }
        if !next.is_empty() {
            fronts.push(next);
        }
        i += 1;
    }

    fronts
}

fn dominates(a: &[f64], b: &[f64]) -> bool {
    if a.len() != b.len() || a.is_empty() {
        return false;
    }
    let mut strictly_better = false;
    for (&ai, &bi) in a.iter().zip(b.iter()) {
        if ai > bi {
            return false;
        }
        if ai < bi {
            strictly_better = true;
        }
    }
    strictly_better
}

fn assign_crowding_distance_for_indices(population: &mut [Individual], front: &[usize]) {
    if front.is_empty() {
        return;
    }
    for &idx in front {
        population[idx].crowding_distance = 0.0;
    }
    if front.len() <= 2 {
        for &idx in front {
            population[idx].crowding_distance = f64::INFINITY;
        }
        return;
    }
    let m = population[front[0]].objectives.len();
    for obj in 0..m {
        let mut sorted = front.to_vec();
        sorted.sort_by(|&a, &b| {
            population[a].objectives[obj].total_cmp(&population[b].objectives[obj])
        });
        let first = sorted[0];
        let last = sorted[sorted.len() - 1];
        population[first].crowding_distance = f64::INFINITY;
        population[last].crowding_distance = f64::INFINITY;
        let min_obj = population[first].objectives[obj];
        let max_obj = population[last].objectives[obj];
        let span = max_obj - min_obj;
        if span <= 0.0 || !span.is_finite() {
            continue;
        }
        for window in sorted.windows(3) {
            let mid = window[1];
            if population[mid].crowding_distance.is_infinite() {
                continue;
            }
            let prev = population[window[0]].objectives[obj];
            let next = population[window[2]].objectives[obj];
            population[mid].crowding_distance += (next - prev) / span;
        }
    }
}

fn nsga3_niching(
    selected: &[Individual],
    splitting_front: Vec<Individual>,
    slots: usize,
    config: &NsgaConfig,
) -> Vec<Individual> {
    if slots == 0 || splitting_front.is_empty() {
        return Vec::new();
    }
    let m = splitting_front[0].objectives.len();
    if m <= 1 {
        let mut sorted = splitting_front;
        sorted.sort_by(compare_individuals);
        return sorted.into_iter().take(slots).collect();
    }
    let partitions = if config.reference_partitions > 0 {
        config.reference_partitions
    } else {
        default_reference_partitions(m)
    };
    let references = reference_directions(m, partitions);
    if references.is_empty() {
        let mut sorted = splitting_front;
        sorted.sort_by(compare_individuals);
        return sorted.into_iter().take(slots).collect();
    }

    let mut all = selected.to_vec();
    all.extend(splitting_front.iter().cloned());
    let (ideal, nadir) = objective_ranges(&all, m);
    let mut niche_counts = vec![0usize; references.len()];
    for ind in selected {
        let norm = normalise_objectives(&ind.objectives, &ideal, &nadir);
        let (niche, _) = nearest_reference(&norm, &references);
        niche_counts[niche] += 1;
    }

    let mut candidates: Vec<(Individual, usize, f64)> = splitting_front
        .into_iter()
        .map(|ind| {
            let norm = normalise_objectives(&ind.objectives, &ideal, &nadir);
            let (niche, dist) = nearest_reference(&norm, &references);
            (ind, niche, dist)
        })
        .collect();

    let mut chosen = Vec::with_capacity(slots);
    while chosen.len() < slots && !candidates.is_empty() {
        let Some(target_niche) = references
            .iter()
            .enumerate()
            .filter(|(idx, _)| candidates.iter().any(|(_, niche, _)| niche == idx))
            .min_by_key(|(idx, _)| niche_counts[*idx])
            .map(|(idx, _)| idx)
        else {
            break;
        };

        let best_idx = candidates
            .iter()
            .enumerate()
            .filter(|(_, (_, niche, _))| *niche == target_niche)
            .min_by(|(_, a), (_, b)| a.2.total_cmp(&b.2))
            .map(|(idx, _)| idx)
            .unwrap_or(0);
        let (ind, niche, _) = candidates.swap_remove(best_idx);
        niche_counts[niche] += 1;
        chosen.push(ind);
    }
    chosen
}

fn objective_ranges(population: &[Individual], m: usize) -> (Vec<f64>, Vec<f64>) {
    let mut ideal = vec![f64::INFINITY; m];
    let mut nadir = vec![f64::NEG_INFINITY; m];
    for ind in population {
        for j in 0..m {
            ideal[j] = ideal[j].min(ind.objectives[j]);
            nadir[j] = nadir[j].max(ind.objectives[j]);
        }
    }
    (ideal, nadir)
}

fn normalise_objectives(objectives: &[f64], ideal: &[f64], nadir: &[f64]) -> Vec<f64> {
    objectives
        .iter()
        .enumerate()
        .map(|(i, &v)| {
            let span = nadir[i] - ideal[i];
            if span > 0.0 && span.is_finite() {
                ((v - ideal[i]) / span).max(0.0)
            } else {
                0.0
            }
        })
        .collect()
}

fn nearest_reference(point: &[f64], references: &[Vec<f64>]) -> (usize, f64) {
    references
        .iter()
        .enumerate()
        .map(|(idx, reference)| (idx, perpendicular_distance(point, reference)))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .unwrap_or((0, 0.0))
}

fn perpendicular_distance(point: &[f64], reference: &[f64]) -> f64 {
    let ref_norm_sq = reference.iter().map(|r| r * r).sum::<f64>();
    if ref_norm_sq <= 0.0 {
        return point.iter().map(|v| v * v).sum::<f64>().sqrt();
    }
    let dot = point
        .iter()
        .zip(reference.iter())
        .map(|(p, r)| p * r)
        .sum::<f64>();
    let scale = dot / ref_norm_sq;
    point
        .iter()
        .zip(reference.iter())
        .map(|(p, r)| {
            let d = p - scale * r;
            d * d
        })
        .sum::<f64>()
        .sqrt()
}

fn default_reference_partitions(m: usize) -> usize {
    match m {
        0 | 1 => 1,
        2 => 24,
        3 => 12,
        4 => 6,
        _ => 3,
    }
}

fn reference_directions(m: usize, partitions: usize) -> Vec<Vec<f64>> {
    if m == 0 {
        return Vec::new();
    }
    if m == 1 {
        return vec![vec![1.0]];
    }
    let h = partitions.max(1);
    let mut out = Vec::new();
    let mut current = vec![0usize; m];
    reference_directions_rec(m, h, h, 0, &mut current, &mut out);
    out
}

fn reference_directions_rec(
    m: usize,
    h: usize,
    left: usize,
    depth: usize,
    current: &mut [usize],
    out: &mut Vec<Vec<f64>>,
) {
    if depth == m - 1 {
        current[depth] = left;
        out.push(current.iter().map(|&v| v as f64 / h as f64).collect());
        return;
    }
    for value in 0..=left {
        current[depth] = value;
        reference_directions_rec(m, h, left - value, depth + 1, current, out);
    }
}

fn to_solutions(population: &[Individual]) -> Vec<ParetoSolution> {
    population
        .iter()
        .map(|ind| ParetoSolution {
            x: ind.x.clone(),
            objectives: ind.objectives.clone(),
            rank: ind.rank,
            crowding_distance: ind.crowding_distance,
        })
        .collect()
}

fn compare_individuals(a: &Individual, b: &Individual) -> std::cmp::Ordering {
    a.rank
        .cmp(&b.rank)
        .then_with(|| b.crowding_distance.total_cmp(&a.crowding_distance))
        .then_with(|| sum_objectives(&a.objectives).total_cmp(&sum_objectives(&b.objectives)))
}

fn compare_solutions(a: &ParetoSolution, b: &ParetoSolution) -> std::cmp::Ordering {
    a.rank
        .cmp(&b.rank)
        .then_with(|| b.crowding_distance.total_cmp(&a.crowding_distance))
        .then_with(|| sum_objectives(&a.objectives).total_cmp(&sum_objectives(&b.objectives)))
}

fn sum_objectives(objectives: &[f64]) -> f64 {
    objectives.iter().sum::<f64>()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two_objective_tradeoff(x: &Array1<f64>) -> Vec<f64> {
        vec![x[0] * x[0], (x[0] - 2.0).powi(2)]
    }

    #[test]
    fn nsga2_finds_both_ends_of_simple_front() {
        let report = nsga2(
            &two_objective_tradeoff,
            NsgaConfig {
                bounds: vec![(0.0, 2.0)],
                population_size: 64,
                maxeval: 1_024,
                seed: Some(11),
                ..Default::default()
            },
        )
        .expect("NSGA-II should run");

        assert!(report.pareto_front.len() > 20);
        assert!(
            report.pareto_front.iter().any(|s| s.objectives[0] < 1e-3),
            "front should include the f1 optimum"
        );
        assert!(
            report.pareto_front.iter().any(|s| s.objectives[1] < 1e-3),
            "front should include the f2 optimum"
        );
    }

    #[test]
    fn nsga3_handles_three_objective_front() {
        let f = |x: &Array1<f64>| vec![x[0].powi(2), (x[0] - 1.0).powi(2), (x[0] - 2.0).powi(2)];
        let report = nsga3(
            &f,
            NsgaConfig {
                bounds: vec![(0.0, 2.0)],
                population_size: 72,
                maxeval: 1_200,
                reference_partitions: 8,
                seed: Some(19),
                ..Default::default()
            },
        )
        .expect("NSGA-III should run");

        assert!(report.pareto_front.len() > 10);
        assert!(report.pareto_front.iter().any(|s| s.x[0] < 0.1));
        assert!(report.pareto_front.iter().any(|s| s.x[0] > 1.9));
    }

    #[test]
    fn dominance_requires_strict_improvement() {
        assert!(dominates(&[1.0, 2.0], &[1.0, 3.0]));
        assert!(!dominates(&[1.0, 2.0], &[1.0, 2.0]));
        assert!(!dominates(&[2.0, 1.0], &[1.0, 2.0]));
    }
}
