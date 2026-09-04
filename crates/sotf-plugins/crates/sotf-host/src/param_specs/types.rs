#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ParamType {
    Float {
        default: f64,
        min: f64,
        max: f64,
        step: f64,
    },
    Int {
        default: i64,
        min: i64,
        max: i64,
        step: i64,
    },
    Bool {
        default: bool,
        true_label: &'static str,
        false_label: &'static str,
    },
    Choice {
        default_index: usize,
        labels: &'static [&'static str],
    },
    FilePath,
}

/// Resolve a UI choice index to its label. Used by factory parameter
/// structs that store the canonical label but must also accept the integer
/// index the toolbar sends for `ParamSpec::choice` controls. Unknown indices
/// are `None` (callers turn this into a hard error, never a silent default).
pub fn choice_label_from_index<'a>(labels: &[&'a str], index: u64) -> Option<&'a str> {
    labels.get(index as usize).copied()
}

/// Define a serde `deserialize_with` function for a `String` field storing a
/// choice label while also accepting the integer index the toolbar sends for
/// `ParamSpec::choice` controls. Unknown indices are hard errors.
/// Requires `serde` in scope at the use site.
#[macro_export]
macro_rules! define_choice_string_deserializer {
    ($fn_name:ident, $labels:expr) => {
        fn $fn_name<'de, D>(deserializer: D) -> Result<String, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            struct ChoiceVisitor;
            impl<'de> serde::de::Visitor<'de> for ChoiceVisitor {
                type Value = String;
                fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                    formatter.write_str("a choice label or index")
                }
                fn visit_str<E>(self, value: &str) -> Result<String, E>
                where
                    E: serde::de::Error,
                {
                    Ok(value.to_owned())
                }
                fn visit_u64<E>(self, value: u64) -> Result<String, E>
                where
                    E: serde::de::Error,
                {
                    $crate::param_specs::choice_label_from_index($labels, value)
                        .map(str::to_owned)
                        .ok_or_else(|| {
                            E::custom(format!(
                                "invalid choice index {value}; expected 0..{}",
                                $labels.len()
                            ))
                        })
                }
                fn visit_i64<E>(self, value: i64) -> Result<String, E>
                where
                    E: serde::de::Error,
                {
                    u64::try_from(value)
                        .ok()
                        .and_then(|index| {
                            $crate::param_specs::choice_label_from_index($labels, index)
                        })
                        .map(str::to_owned)
                        .ok_or_else(|| {
                            E::custom(format!(
                                "invalid choice index {value}; expected 0..{}",
                                $labels.len()
                            ))
                        })
                }
                fn visit_f64<E>(self, value: f64) -> Result<String, E>
                where
                    E: serde::de::Error,
                {
                    // Integral floats from the daemon wire format (`1.0`)
                    // resolve exactly like their integer index.
                    if value.is_finite() && value.fract() == 0.0 && value >= 0.0 {
                        self.visit_u64(value as u64)
                    } else {
                        Err(E::custom(format!(
                            "invalid choice index {value}; expected an integral 0..{}",
                            $labels.len()
                        )))
                    }
                }
            }
            deserializer.deserialize_any(ChoiceVisitor)
        }
    };
}

/// Define a serde `deserialize_with` function for an `Option<String>` field
/// storing a choice label while also accepting the integer index. `null`
/// maps to `None`; unknown indices are hard errors. Requires `serde` in
/// scope at the use site.
#[macro_export]
macro_rules! define_choice_string_option_deserializer {
    ($fn_name:ident, $labels:expr) => {
        fn $fn_name<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            struct ChoiceVisitor;
            impl<'de> serde::de::Visitor<'de> for ChoiceVisitor {
                type Value = Option<String>;
                fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                    formatter.write_str("a choice label, index, or null")
                }
                fn visit_none<E>(self) -> Result<Option<String>, E>
                where
                    E: serde::de::Error,
                {
                    Ok(None)
                }
                fn visit_unit<E>(self) -> Result<Option<String>, E>
                where
                    E: serde::de::Error,
                {
                    Ok(None)
                }
                fn visit_str<E>(self, value: &str) -> Result<Option<String>, E>
                where
                    E: serde::de::Error,
                {
                    Ok(Some(value.to_owned()))
                }
                fn visit_u64<E>(self, value: u64) -> Result<Option<String>, E>
                where
                    E: serde::de::Error,
                {
                    $crate::param_specs::choice_label_from_index($labels, value)
                        .map(|label| Some(label.to_owned()))
                        .ok_or_else(|| {
                            E::custom(format!(
                                "invalid choice index {value}; expected 0..{}",
                                $labels.len()
                            ))
                        })
                }
                fn visit_i64<E>(self, value: i64) -> Result<Option<String>, E>
                where
                    E: serde::de::Error,
                {
                    u64::try_from(value)
                        .ok()
                        .and_then(|index| {
                            $crate::param_specs::choice_label_from_index($labels, index)
                        })
                        .map(|label| Some(label.to_owned()))
                        .ok_or_else(|| {
                            E::custom(format!(
                                "invalid choice index {value}; expected 0..{}",
                                $labels.len()
                            ))
                        })
                }
                fn visit_f64<E>(self, value: f64) -> Result<Option<String>, E>
                where
                    E: serde::de::Error,
                {
                    if value.is_finite() && value.fract() == 0.0 && value >= 0.0 {
                        self.visit_u64(value as u64)
                    } else {
                        Err(E::custom(format!(
                            "invalid choice index {value}; expected an integral 0..{}",
                            $labels.len()
                        )))
                    }
                }
            }
            deserializer.deserialize_any(ChoiceVisitor)
        }
    };
}

/// Define a serde `deserialize_with` function for a `usize` field storing a
/// choice index while also accepting the string label. Unknown labels and
/// out-of-range indices are hard errors. Requires `serde` in scope.
#[macro_export]
macro_rules! define_choice_index_deserializer {
    ($fn_name:ident, $labels:expr) => {
        fn $fn_name<'de, D>(deserializer: D) -> Result<usize, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            struct ChoiceVisitor;
            impl<'de> serde::de::Visitor<'de> for ChoiceVisitor {
                type Value = usize;
                fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                    formatter.write_str("a choice index or label")
                }
                fn visit_u64<E>(self, value: u64) -> Result<usize, E>
                where
                    E: serde::de::Error,
                {
                    usize::try_from(value)
                        .ok()
                        .filter(|index| *index < $labels.len())
                        .ok_or_else(|| {
                            E::custom(format!(
                                "invalid choice index {value}; expected 0..{}",
                                $labels.len()
                            ))
                        })
                }
                fn visit_i64<E>(self, value: i64) -> Result<usize, E>
                where
                    E: serde::de::Error,
                {
                    usize::try_from(value)
                        .ok()
                        .filter(|index| *index < $labels.len())
                        .ok_or_else(|| {
                            E::custom(format!(
                                "invalid choice index {value}; expected 0..{}",
                                $labels.len()
                            ))
                        })
                }
                fn visit_str<E>(self, value: &str) -> Result<usize, E>
                where
                    E: serde::de::Error,
                {
                    $crate::param_specs::choice_index_from_label($labels, value)
                        .ok_or_else(|| E::custom(format!("unknown choice label {value:?}")))
                }
                fn visit_f64<E>(self, value: f64) -> Result<usize, E>
                where
                    E: serde::de::Error,
                {
                    // The toolbar/daemon wire format carries integral floats
                    // (e.g. `1.0`); accept them exactly, reject fractions.
                    if value.is_finite() && value.fract() == 0.0 && value >= 0.0 {
                        self.visit_u64(value as u64)
                    } else {
                        Err(E::custom(format!(
                            "invalid choice index {value}; expected an integral 0..{}",
                            $labels.len()
                        )))
                    }
                }
            }
            deserializer.deserialize_any(ChoiceVisitor)
        }
    };
}

/// Resolve a choice label to its index. Exact match wins; a case-insensitive
/// fallback covers benign case drift between UI labels and canonical names
/// (e.g. `"HRTF"` vs `"Hrtf"`). Returns `None` for unknown labels.
pub fn choice_index_from_label(labels: &[&str], label: &str) -> Option<usize> {
    labels
        .iter()
        .position(|candidate| *candidate == label)
        .or_else(|| {
            labels
                .iter()
                .position(|candidate| candidate.eq_ignore_ascii_case(label))
        })
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum UpdateMode {
    /// Parameter can be updated without rebuilding the plugin (zero-dropout).
    #[default]
    Realtime,
    /// Parameter change requires rebuilding the plugin (e.g., channel count change).
    Structural,
}

/// UI layout category for automatic 3-column plugin rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamCategory {
    /// Left column: structural params, mode selectors, channel config.
    Setup,
    /// Center-top: main controls users adjust frequently.
    Primary,
    /// Center-bottom with tabs: advanced/fine-tuning params. Tab name groups them.
    Secondary(&'static str),
    /// Right column: meter, AutoGain, Mix, Makeup.
    Output,
    /// Center-bottom tab: bypass toggles, debug params.
    Diagnostic,
}
