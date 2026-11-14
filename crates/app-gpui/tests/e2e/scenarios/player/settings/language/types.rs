/// Supported language
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum Language {
    #[default]
    English,
    Spanish,
    French,
    German,
    Japanese,
    Chinese,
    Portuguese,
    Russian,
}

/// Date format preference
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(
    clippy::upper_case_acronyms,
    reason = "Preserve the existing protocol and UI identifier spelling"
)]
pub(super) enum DateFormat {
    #[default]
    MDY, // MM/DD/YYYY (US)
    DMY, // DD/MM/YYYY (Europe)
    YMD, // YYYY-MM-DD (ISO)
}

/// Time format preference
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum TimeFormat {
    #[default]
    Hour12,
    Hour24,
}

/// Number format preference
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(
    clippy::enum_variant_names,
    reason = "The existing variants mirror their public UI operations"
)]
pub(super) enum NumberFormat {
    #[default]
    CommaSeparator, // 1,000.00
    DotSeparator,   // 1.000,00
    SpaceSeparator, // 1 000,00
}
