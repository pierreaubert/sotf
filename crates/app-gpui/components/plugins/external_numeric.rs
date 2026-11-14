//! Numeric widget limits derived from native hosted parameter metadata.

use sotf_plugins::ParameterValue;

#[derive(Clone, Copy, Debug)]
pub(super) struct NumericControl {
    pub current: f64,
    pub min: f64,
    pub max: f64,
    pub integer: bool,
    pub slider_range: Option<(f32, f32)>,
}

impl NumericControl {
    /// Validate native readback without clamping it into a fabricated value.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "UI controls and graph coordinates use approximate floating-point parameter values"
    )]
    pub fn from_native(
        value: &ParameterValue,
        min: Option<&ParameterValue>,
        max: Option<&ParameterValue>,
    ) -> Result<Self, &'static str> {
        let number = |value: &ParameterValue| match value {
            ParameterValue::Float(value) => Some(f64::from(*value)),
            ParameterValue::Int(value) => Some(f64::from(*value)),
            _ => None,
        };
        let current = number(value).ok_or("Not a numeric parameter")?;
        if !current.is_finite() {
            return Err("Native value is not finite");
        }
        let integer = matches!(value, ParameterValue::Int(_));
        let (type_min, type_max) = if integer {
            (f64::from(i32::MIN), f64::from(i32::MAX))
        } else {
            (-f64::from(f32::MAX), f64::from(f32::MAX))
        };
        let bound = |value: Option<&ParameterValue>, fallback| match value {
            Some(value) => number(value)
                .filter(|value| value.is_finite())
                .ok_or("Invalid native parameter limits"),
            None => Ok(fallback),
        };
        let mut lower = bound(min, type_min)?.max(type_min);
        let mut upper = bound(max, type_max)?.min(type_max);
        if integer {
            lower = lower.ceil();
            upper = upper.floor();
        }
        if lower > upper {
            return Err("Invalid native parameter limits");
        }
        if current < lower || current > upper {
            return Err("Native value is outside its parameter limits");
        }
        // A slider needs two native endpoints. Type limits only constrain exact
        // input conversion; they do not invent a useful native slider range.
        // Large integers cannot be represented exactly by the f32 slider.
        let slider_range = (min.is_some()
            && max.is_some()
            && lower < upper
            && (!integer || (lower >= -16_777_216.0 && upper <= 16_777_216.0)))
            .then_some((lower as f32, upper as f32));
        Ok(Self {
            current,
            min: lower,
            max: upper,
            integer,
            slider_range,
        })
    }

    /// Convert exact input only when representable and inside the native limits.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "UI controls and graph coordinates use approximate floating-point parameter values"
    )]
    pub fn encode(self, value: f64) -> Option<ParameterValue> {
        if !value.is_finite() || value < self.min || value > self.max {
            return None;
        }
        if self.integer {
            let value = value.round();
            (value >= self.min && value <= self.max).then_some(ParameterValue::Int(value as i32))
        } else {
            Some(ParameterValue::Float(value as f32))
        }
    }
}
