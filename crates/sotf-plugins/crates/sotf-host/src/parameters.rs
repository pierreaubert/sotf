// ============================================================================
// Plugin Parameter System
// ============================================================================

use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;

/// Unique identifier for a parameter
#[derive(Debug, Clone, Hash, Eq, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ParameterId(pub Arc<str>);

impl From<&str> for ParameterId {
    fn from(s: &str) -> Self {
        ParameterId(Arc::from(s))
    }
}

impl From<String> for ParameterId {
    fn from(s: String) -> Self {
        ParameterId(Arc::from(s))
    }
}

impl fmt::Display for ParameterId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl ParameterId {
    /// Get the parameter ID as a string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume the wrapper and return the inner `Arc<str>`.
    pub fn into_arc(self) -> Arc<str> {
        self.0
    }
}

/// Parameter value types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ParameterValue {
    Float(f32),
    Int(i32),
    Bool(bool),
    String(String),
}

impl ParameterValue {
    /// Get as float, returns None if not a float
    pub fn as_float(&self) -> Option<f32> {
        match self {
            ParameterValue::Float(v) => Some(*v),
            _ => None,
        }
    }

    /// Get as int, returns None if not an int
    pub fn as_int(&self) -> Option<i32> {
        match self {
            ParameterValue::Int(v) => Some(*v),
            _ => None,
        }
    }

    /// Get as bool, returns None if not a bool
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            ParameterValue::Bool(v) => Some(*v),
            _ => None,
        }
    }

    /// Parse a string value into a ParameterValue.
    ///
    /// Detection order: Bool ("true"/"false") → Int (no decimal point) → Float → String.
    /// Values containing a decimal point are never parsed as Int, preventing
    /// type mismatches when Float values happen to be whole numbers (e.g. "-18.0").
    pub fn parse(value: &str) -> Self {
        if value == "true" {
            return Self::Bool(true);
        }
        if value == "false" {
            return Self::Bool(false);
        }
        // Only try integer when the string has no decimal point.
        if !value.contains('.')
            && let Ok(i) = value.parse::<i32>()
        {
            return Self::Int(i);
        }
        if let Ok(f) = value.parse::<f32>() {
            return Self::Float(f);
        }
        Self::String(value.to_string())
    }

    /// Get as string, returns None if not a string
    pub fn as_string(&self) -> Option<&str> {
        match self {
            ParameterValue::String(v) => Some(v),
            _ => None,
        }
    }
}

impl fmt::Display for ParameterValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParameterValue::Float(v) => write!(f, "{:.2}", v),
            ParameterValue::Int(v) => write!(f, "{}", v),
            ParameterValue::Bool(v) => write!(f, "{}", v),
            ParameterValue::String(v) => write!(f, "{}", v),
        }
    }
}

/// Importance level for UI generation and responsive design
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParameterImportance {
    /// Always visible, core functionality
    Critical,
    /// Often used, visible in detailed view
    Useful,
    /// Rarely used, visible in expert mode or expanded view
    FineTuning,
}

/// Parameter definition with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameter {
    /// Unique identifier
    pub id: ParameterId,
    /// Human-readable name
    pub name: String,
    /// Optional description
    pub description: Option<String>,
    /// Logical group name (e.g., "Dynamics", "EQ")
    pub group: String,
    /// Importance level
    pub importance: ParameterImportance,
    /// Default value
    pub default_value: ParameterValue,
    /// Minimum value (for numeric parameters)
    pub min_value: Option<ParameterValue>,
    /// Maximum value (for numeric parameters)
    pub max_value: Option<ParameterValue>,
    /// Unit string (e.g., "dB", "Hz", "%")
    pub unit: String,
    /// Whether this parameter uses logarithmic scaling
    pub logarithmic: bool,
}

impl Parameter {
    /// Create a new float parameter
    pub fn new_float(id: &str, name: &str, default: f32, min: f32, max: f32) -> Self {
        Self {
            id: ParameterId::from(id),
            name: name.to_string(),
            description: None,
            group: "General".to_string(),
            importance: ParameterImportance::Useful,
            default_value: ParameterValue::Float(default),
            min_value: Some(ParameterValue::Float(min)),
            max_value: Some(ParameterValue::Float(max)),
            unit: String::new(),
            logarithmic: false,
        }
    }

    /// Create a new integer parameter
    pub fn new_int(id: &str, name: &str, default: i32, min: i32, max: i32) -> Self {
        Self {
            id: ParameterId::from(id),
            name: name.to_string(),
            description: None,
            group: "General".to_string(),
            importance: ParameterImportance::Useful,
            default_value: ParameterValue::Int(default),
            min_value: Some(ParameterValue::Int(min)),
            max_value: Some(ParameterValue::Int(max)),
            unit: String::new(),
            logarithmic: false,
        }
    }

    /// Create a new boolean parameter
    pub fn new_bool(id: &str, name: &str, default: bool) -> Self {
        Self {
            id: ParameterId::from(id),
            name: name.to_string(),
            description: None,
            group: "General".to_string(),
            importance: ParameterImportance::Useful,
            default_value: ParameterValue::Bool(default),
            min_value: None,
            max_value: None,
            unit: String::new(),
            logarithmic: false,
        }
    }

    /// Create a new string parameter (for JSON-serialized complex types)
    pub fn new_string(id: &str, name: &str, default: String) -> Self {
        Self {
            id: ParameterId::from(id),
            name: name.to_string(),
            description: None,
            group: "General".to_string(),
            importance: ParameterImportance::Useful,
            default_value: ParameterValue::String(default),
            min_value: None,
            max_value: None,
            unit: String::new(),
            logarithmic: false,
        }
    }

    /// Set unit string
    pub fn with_unit(mut self, unit: &str) -> Self {
        self.unit = unit.to_string();
        self
    }

    /// Set logarithmic scaling
    pub fn with_logarithmic(mut self, logarithmic: bool) -> Self {
        self.logarithmic = logarithmic;
        self
    }

    /// Set description
    pub fn with_description(mut self, description: &str) -> Self {
        self.description = Some(description.to_string());
        self
    }

    /// Set group
    pub fn with_group(mut self, group: &str) -> Self {
        self.group = group.to_string();
        self
    }

    /// Set importance
    pub fn with_importance(mut self, importance: ParameterImportance) -> Self {
        self.importance = importance;
        self
    }

    /// Build the parameter (return self for method chaining)
    #[inline]
    pub fn build(self) -> Self {
        self
    }

    /// Validate a value against this parameter's constraints
    pub fn validate(&self, value: &ParameterValue) -> Result<(), String> {
        // Check type matches
        match (&self.default_value, value) {
            (ParameterValue::Float(_), ParameterValue::Float(v)) => {
                if v.is_nan() {
                    return Err("Value is NaN".to_string());
                }
                if v.is_infinite() {
                    return Err("Value is infinite".to_string());
                }
                if let Some(ParameterValue::Float(min)) = self.min_value
                    && *v < min
                {
                    return Err(format!("Value {} is below minimum {}", v, min));
                }
                if let Some(ParameterValue::Float(max)) = self.max_value
                    && *v > max
                {
                    return Err(format!("Value {} is above maximum {}", v, max));
                }
                Ok(())
            }
            (ParameterValue::Int(_), ParameterValue::Int(v)) => {
                if let Some(ParameterValue::Int(min)) = self.min_value
                    && *v < min
                {
                    return Err(format!("Value {} is below minimum {}", v, min));
                }
                if let Some(ParameterValue::Int(max)) = self.max_value
                    && *v > max
                {
                    return Err(format!("Value {} is above maximum {}", v, max));
                }
                Ok(())
            }
            (ParameterValue::Bool(_), ParameterValue::Bool(_)) => Ok(()),
            (ParameterValue::String(_), ParameterValue::String(_)) => Ok(()),
            _ => Err("Parameter type mismatch".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_float_with_decimal() {
        assert_eq!(ParameterValue::parse("-18.0"), ParameterValue::Float(-18.0));
        assert_eq!(ParameterValue::parse("0.0"), ParameterValue::Float(0.0));
        assert_eq!(ParameterValue::parse("1.5"), ParameterValue::Float(1.5));
        assert_eq!(ParameterValue::parse("-0.5"), ParameterValue::Float(-0.5));
    }

    #[test]
    fn parse_int_without_decimal() {
        assert_eq!(ParameterValue::parse("42"), ParameterValue::Int(42));
        assert_eq!(ParameterValue::parse("-18"), ParameterValue::Int(-18));
        assert_eq!(ParameterValue::parse("0"), ParameterValue::Int(0));
    }

    #[test]
    fn parse_bool() {
        assert_eq!(ParameterValue::parse("true"), ParameterValue::Bool(true));
        assert_eq!(ParameterValue::parse("false"), ParameterValue::Bool(false));
    }

    #[test]
    fn parse_string_fallback() {
        assert_eq!(
            ParameterValue::parse("[{\"state\":\"normal\"}]"),
            ParameterValue::String("[{\"state\":\"normal\"}]".to_string())
        );
        assert_eq!(
            ParameterValue::parse("hello"),
            ParameterValue::String("hello".to_string())
        );
    }

    #[test]
    fn parse_scientific_notation_as_float() {
        assert_eq!(ParameterValue::parse("1.5e2"), ParameterValue::Float(150.0));
    }

    #[test]
    fn parse_decimal_point_prevents_int() {
        // "100.0" must parse as Float, not Int
        assert_eq!(ParameterValue::parse("100.0"), ParameterValue::Float(100.0));
        assert_eq!(ParameterValue::parse("1.0"), ParameterValue::Float(1.0));
    }

    #[test]
    fn parameter_default_group_is_general() {
        let p = Parameter::new_float("freq", "Frequency", 1000.0, 20.0, 20_000.0);
        assert_eq!(p.group, "General");
    }

    #[test]
    fn parameter_grouping_helpers_chain() {
        let p = Parameter::new_float("freq", "Frequency", 1000.0, 20.0, 20_000.0)
            .with_group("EQ")
            .with_importance(ParameterImportance::Critical)
            .with_unit("Hz")
            .with_description("Center frequency of the filter")
            .with_logarithmic(true)
            .build();

        assert_eq!(p.group, "EQ");
        assert_eq!(p.importance, ParameterImportance::Critical);
        assert_eq!(p.unit, "Hz");
        assert_eq!(
            p.description.as_deref(),
            Some("Center frequency of the filter")
        );
        assert!(p.logarithmic);
    }

    #[test]
    fn parameter_groups_are_independent_between_instances() {
        let eq = Parameter::new_float("freq", "Frequency", 1000.0, 20.0, 20_000.0).with_group("EQ");
        let dynamics = Parameter::new_float("threshold", "Threshold", -18.0, -60.0, 0.0)
            .with_group("Dynamics");

        assert_eq!(eq.group, "EQ");
        assert_eq!(dynamics.group, "Dynamics");
    }
}
