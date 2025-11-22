// ============================================================================
// Plugin Parameter System
// ============================================================================

use std::fmt;

/// Unique identifier for a parameter
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct ParameterId(pub String);

impl From<&str> for ParameterId {
    fn from(s: &str) -> Self {
        ParameterId(s.to_string())
    }
}

impl fmt::Display for ParameterId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Parameter value types
#[derive(Debug, Clone, PartialEq)]
pub enum ParameterValue {
    Float(f32),
    Int(i32),
    Bool(bool),
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
}

impl fmt::Display for ParameterValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParameterValue::Float(v) => write!(f, "{:.2}", v),
            ParameterValue::Int(v) => write!(f, "{}", v),
            ParameterValue::Bool(v) => write!(f, "{}", v),
        }
    }
}

/// Parameter definition with metadata
#[derive(Debug, Clone)]
pub struct Parameter {
    /// Unique identifier
    pub id: ParameterId,
    /// Human-readable name
    pub name: String,
    /// Optional description
    pub description: Option<String>,
    /// Default value
    pub default_value: ParameterValue,
    /// Minimum value (for numeric parameters)
    pub min_value: Option<ParameterValue>,
    /// Maximum value (for numeric parameters)
    pub max_value: Option<ParameterValue>,
}

impl Parameter {
    /// Create a new float parameter
    pub fn new_float(id: &str, name: &str, default: f32, min: f32, max: f32) -> Self {
        Self {
            id: ParameterId::from(id),
            name: name.to_string(),
            description: None,
            default_value: ParameterValue::Float(default),
            min_value: Some(ParameterValue::Float(min)),
            max_value: Some(ParameterValue::Float(max)),
        }
    }

    /// Create a new integer parameter
    pub fn new_int(id: &str, name: &str, default: i32, min: i32, max: i32) -> Self {
        Self {
            id: ParameterId::from(id),
            name: name.to_string(),
            description: None,
            default_value: ParameterValue::Int(default),
            min_value: Some(ParameterValue::Int(min)),
            max_value: Some(ParameterValue::Int(max)),
        }
    }

    /// Create a new boolean parameter
    pub fn new_bool(id: &str, name: &str, default: bool) -> Self {
        Self {
            id: ParameterId::from(id),
            name: name.to_string(),
            description: None,
            default_value: ParameterValue::Bool(default),
            min_value: None,
            max_value: None,
        }
    }

    /// Set description
    pub fn with_description(mut self, description: &str) -> Self {
        self.description = Some(description.to_string());
        self
    }

    /// Validate a value against this parameter's constraints
    pub fn validate(&self, value: &ParameterValue) -> Result<(), String> {
        // Check type matches
        match (&self.default_value, value) {
            (ParameterValue::Float(_), ParameterValue::Float(v)) => {
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
            _ => Err("Parameter type mismatch".to_string()),
        }
    }
}
