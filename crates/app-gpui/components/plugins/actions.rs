use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct UpdatePluginParam {
    pub plugin_idx: usize,
    pub param_idx: usize,
    pub value: f64,
}

#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct SelectPluginParam {
    pub plugin_idx: usize,
    pub param_idx: usize,
}

#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct ResetPluginParam {
    pub plugin_idx: usize,
    pub param_idx: usize,
}

#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct StartKnobDrag {
    pub plugin_idx: usize,
    pub param_idx: usize,
    pub start_y: f32,
    pub start_value: f64,
    pub min: f64,
    pub max: f64,
}

#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct ToggleUpmixerConfig {
    pub open: bool,
}

impl gpui::Action for UpdatePluginParam {
    fn boxed_clone(&self) -> Box<dyn gpui::Action> {
        Box::new(self.clone())
    }
    fn partial_eq(&self, other: &dyn gpui::Action) -> bool {
        other
            .as_any()
            .downcast_ref::<Self>() == Some(self)
    }
    fn name(&self) -> &'static str {
        "UpdatePluginParam"
    }
    fn name_for_type() -> &'static str {
        "UpdatePluginParam"
    }
    fn build(_: serde_json::Value) -> anyhow::Result<Box<dyn gpui::Action>> {
        Err(anyhow::anyhow!("Not supported via keymaps"))
    }
}

impl gpui::Action for SelectPluginParam {
    fn boxed_clone(&self) -> Box<dyn gpui::Action> {
        Box::new(self.clone())
    }
    fn partial_eq(&self, other: &dyn gpui::Action) -> bool {
        other
            .as_any()
            .downcast_ref::<Self>() == Some(self)
    }
    fn name(&self) -> &'static str {
        "SelectPluginParam"
    }
    fn name_for_type() -> &'static str {
        "SelectPluginParam"
    }
    fn build(_: serde_json::Value) -> anyhow::Result<Box<dyn gpui::Action>> {
        Err(anyhow::anyhow!("Not supported via keymaps"))
    }
}

impl gpui::Action for ResetPluginParam {
    fn boxed_clone(&self) -> Box<dyn gpui::Action> {
        Box::new(self.clone())
    }
    fn partial_eq(&self, other: &dyn gpui::Action) -> bool {
        other
            .as_any()
            .downcast_ref::<Self>() == Some(self)
    }
    fn name(&self) -> &'static str {
        "ResetPluginParam"
    }
    fn name_for_type() -> &'static str {
        "ResetPluginParam"
    }
    fn build(_: serde_json::Value) -> anyhow::Result<Box<dyn gpui::Action>> {
        Err(anyhow::anyhow!("Not supported via keymaps"))
    }
}

impl gpui::Action for StartKnobDrag {
    fn boxed_clone(&self) -> Box<dyn gpui::Action> {
        Box::new(self.clone())
    }
    fn partial_eq(&self, other: &dyn gpui::Action) -> bool {
        other
            .as_any()
            .downcast_ref::<Self>() == Some(self)
    }
    fn name(&self) -> &'static str {
        "StartKnobDrag"
    }
    fn name_for_type() -> &'static str {
        "StartKnobDrag"
    }
    fn build(_: serde_json::Value) -> anyhow::Result<Box<dyn gpui::Action>> {
        Err(anyhow::anyhow!("Not supported via keymaps"))
    }
}

impl gpui::Action for ToggleUpmixerConfig {
    fn boxed_clone(&self) -> Box<dyn gpui::Action> {
        Box::new(self.clone())
    }
    fn partial_eq(&self, other: &dyn gpui::Action) -> bool {
        other
            .as_any()
            .downcast_ref::<Self>() == Some(self)
    }
    fn name(&self) -> &'static str {
        "ToggleUpmixerConfig"
    }
    fn name_for_type() -> &'static str {
        "ToggleUpmixerConfig"
    }
    fn build(_: serde_json::Value) -> anyhow::Result<Box<dyn gpui::Action>> {
        Err(anyhow::anyhow!("Not supported via keymaps"))
    }
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct OpenSofaFile {
    pub plugin_idx: usize,
}

impl gpui::Action for OpenSofaFile {
    fn boxed_clone(&self) -> Box<dyn gpui::Action> {
        Box::new(self.clone())
    }
    fn partial_eq(&self, other: &dyn gpui::Action) -> bool {
        other
            .as_any()
            .downcast_ref::<Self>() == Some(self)
    }
    fn name(&self) -> &'static str {
        "OpenSofaFile"
    }
    fn name_for_type() -> &'static str {
        "OpenSofaFile"
    }
    fn build(_: serde_json::Value) -> anyhow::Result<Box<dyn gpui::Action>> {
        Err(anyhow::anyhow!("Not supported via keymaps"))
    }
}
