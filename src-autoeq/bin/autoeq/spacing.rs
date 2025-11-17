use autoeq::optim;

/// Check if spacing constraints are met
pub(super) fn check_spacing_constraints(x: &[f64], args: &autoeq::cli::Args) -> bool {
    let peq_model = args.effective_peq_model();
    let (_, adj_spacings) = optim::compute_sorted_freqs_and_adjacent_octave_spacings(x, peq_model);
    let min_adj = adj_spacings.iter().cloned().fold(f64::INFINITY, f64::min);
    min_adj >= args.min_spacing_oct && min_adj.is_finite()
}

/// Print frequency spacing diagnostics and PEQ listing
pub(super) fn print_freq_spacing(x: &[f64], args: &autoeq::cli::Args, label: &str) {
    let peq_model = args.effective_peq_model();
    let (sorted_freqs, adj_spacings) =
        optim::compute_sorted_freqs_and_adjacent_octave_spacings(x, peq_model);
    let min_adj = adj_spacings.iter().cloned().fold(f64::INFINITY, f64::min);
    let freqs_fmt: Vec<String> = sorted_freqs.iter().map(|f| format!("{:.0}", f)).collect();
    let spacings_fmt: Vec<String> = adj_spacings.iter().map(|s| format!("{:.2}", s)).collect();
    if min_adj >= args.min_spacing_oct {
        println!("✅ Spacing diagnostics ({}):", label);
    } else {
        println!("⚠️ Spacing diagnostics ({}):", label);
    }
    println!("  - Sorted center freqs (Hz): [{}]", freqs_fmt.join(", "));
    println!(
        "  - Adjacent spacings (oct):   [{}]",
        spacings_fmt.join(", ")
    );
    if min_adj.is_finite() {
        println!(
            "  - Min adjacent spacing: {:.4} oct (constraint {:.4} oct)",
            min_adj, args.min_spacing_oct
        );
    } else {
        println!("  - Not enough filters to compute spacing.");
    }
    autoeq::x2peq::peq_print_from_x(x, args.effective_peq_model());
}
