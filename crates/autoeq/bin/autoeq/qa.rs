/// Structure to hold QA analysis results
pub(super) struct QaAnalysisResult {
    pub(super) converge_ok: bool,
    pub(super) spacing_ok: bool,
    pub(super) improvement_ok: bool,
    pub(super) improvement_threshold: f64,
    pub(super) pre_value: f64,
    pub(super) post_value: f64,
}

/// Perform QA analysis similar to qa_check.sh
pub(super) fn perform_qa_analysis(
    converged: bool,
    spacing_ok: bool,
    pre_score: Option<f64>,
    post_score: Option<f64>,
    threshold: f64,
) -> QaAnalysisResult {
    let pre_value = pre_score.unwrap_or(f64::NAN);
    let post_value = post_score.unwrap_or(f64::NAN);

    // Check convergence
    let converge_ok = converged;

    // Check spacing (already computed)
    let spacing_check_ok = spacing_ok;

    // Check improvement: post > pre + threshold
    let improvement_threshold = pre_value + threshold;
    let improvement_ok =
        !pre_value.is_nan() && !post_value.is_nan() && post_value > improvement_threshold;

    QaAnalysisResult {
        converge_ok,
        spacing_ok: spacing_check_ok,
        improvement_ok,
        improvement_threshold,
        pre_value,
        post_value,
    }
}

/// Display QA analysis results similar to qa_check.sh
pub(super) fn display_qa_analysis(result: &QaAnalysisResult) {
    println!("Parsed values:");
    println!(
        "  Converge: {} ({})",
        if result.converge_ok { "true" } else { "false" },
        if result.converge_ok { "✓" } else { "✗" }
    );
    println!(
        "  Spacing:  {} ({})",
        if result.spacing_ok { "ok" } else { "ko" },
        if result.spacing_ok { "✓" } else { "✗" }
    );
    println!("  Pre:      {:.3}", result.pre_value);
    println!("  Post:     {:.3}", result.post_value);
    println!(
        "  Improvement: {:.3} > {:.3} + {:.1} = {:.3} ({})",
        result.post_value,
        result.pre_value,
        result.improvement_threshold - result.pre_value,
        result.improvement_threshold,
        if result.improvement_ok { "✓" } else { "✗" }
    );
    println!();

    // Final result
    if result.converge_ok && result.spacing_ok && result.improvement_ok {
        println!("OK");
    } else {
        println!("FAIL");
    }
}
