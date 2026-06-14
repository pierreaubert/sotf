use gpui_ui_kit::SelectOption;

pub(super) fn room_eq_smoothing_options() -> Vec<SelectOption> {
    vec![
        SelectOption::new("0.16666666666666666", "1/6 Oct"),
        SelectOption::new("0.3333333333333333", "1/3 Oct"),
        SelectOption::new("0.5", "1/2 Oct"),
        SelectOption::new("1", "1 Oct"),
        SelectOption::new("0.08333333333333333", "1/12 Oct"),
        SelectOption::new("0.041666666666666664", "1/24 Oct"),
        SelectOption::new("0.020833333333333332", "1/48 Oct"),
        SelectOption::new("0", "Raw"),
    ]
}

pub(super) fn room_eq_smoothing_value(value: f64) -> &'static str {
    const OPTIONS: &[(f64, &str)] = &[
        (1.0 / 6.0, "0.16666666666666666"),
        (1.0 / 3.0, "0.3333333333333333"),
        (0.5, "0.5"),
        (1.0, "1"),
        (1.0 / 12.0, "0.08333333333333333"),
        (1.0 / 24.0, "0.041666666666666664"),
        (1.0 / 48.0, "0.020833333333333332"),
        (0.0, "0"),
    ];
    OPTIONS
        .iter()
        .find(|(candidate, _)| (value - *candidate).abs() < 1.0e-9)
        .map(|(_, key)| *key)
        .unwrap_or("0.16666666666666666")
}
