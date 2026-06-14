use super::types::TuiParamDescriptor;
use super::types::TuiParamSpec;
use super::types::TuiParamType;

pub trait TuiEditablePlugin {
    fn get_descriptors(&self) -> Vec<TuiParamDescriptor>;
    fn get_params(&self) -> Vec<TuiParamSpec>;
    fn adjust_param(&mut self, index: usize, delta: f64) -> bool;
    fn get_value_as_string(&self, index: usize) -> String;
    /// Return the list of choice labels for a Choice parameter.
    /// Returns empty vec for non-Choice params.
    fn get_choice_labels(&self, _index: usize) -> Vec<String> {
        Vec::new()
    }
    /// Set a parameter to an absolute value.
    /// For Float/Int: computes the delta from current value and calls adjust_param.
    /// For Bool: value > 0.5 = true.
    /// For Choice: value is the option index.
    fn set_param(&mut self, index: usize, value: f64) -> bool {
        let current_str = self.get_value_as_string(index);
        let current = current_str.parse::<f64>().unwrap_or(0.0);
        let descriptors = self.get_descriptors();
        if let Some(desc) = descriptors.get(index) {
            match desc.param_type {
                TuiParamType::Float { step, .. } => {
                    // Compute delta in units of what adjust_param expects (delta=1.0 means +step)
                    let delta = (value - current) / step;
                    if delta.abs() > 0.001 {
                        return self.adjust_param(index, delta);
                    }
                }
                TuiParamType::Int { step, .. } => {
                    let delta = (value - current) / step as f64;
                    if delta.abs() > 0.001 {
                        return self.adjust_param(index, delta);
                    }
                }
                TuiParamType::Bool => {
                    let is_true = current_str == "On"
                        || current_str == "true"
                        || current_str == "Linked"
                        || current_str == "Soft"
                        || current_str == "1";
                    let want_true = value > 0.5;
                    if is_true != want_true {
                        return self.adjust_param(index, 1.0);
                    }
                }
                TuiParamType::Choice { count } => {
                    let target = (value as usize).min(count.saturating_sub(1));
                    // Cycle forward until we reach the target
                    for _ in 0..count {
                        let cur = self.get_value_as_string(index);
                        let labels = self.get_choice_labels(index);
                        if let Some(cur_idx) = labels.iter().position(|l| *l == cur) {
                            if cur_idx == target {
                                return true;
                            }
                        }
                        self.adjust_param(index, 1.0);
                    }
                }
            }
        }
        false
    }
}
