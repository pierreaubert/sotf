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
        other.as_any().downcast_ref::<Self>() == Some(self)
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
        other.as_any().downcast_ref::<Self>() == Some(self)
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
        other.as_any().downcast_ref::<Self>() == Some(self)
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
        other.as_any().downcast_ref::<Self>() == Some(self)
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
        other.as_any().downcast_ref::<Self>() == Some(self)
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
        other.as_any().downcast_ref::<Self>() == Some(self)
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

#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct OpenIrFile {
    pub plugin_idx: usize,
}

impl gpui::Action for OpenIrFile {
    fn boxed_clone(&self) -> Box<dyn gpui::Action> {
        Box::new(self.clone())
    }
    fn partial_eq(&self, other: &dyn gpui::Action) -> bool {
        other.as_any().downcast_ref::<Self>() == Some(self)
    }
    fn name(&self) -> &'static str {
        "OpenIrFile"
    }
    fn name_for_type() -> &'static str {
        "OpenIrFile"
    }
    fn build(_: serde_json::Value) -> anyhow::Result<Box<dyn gpui::Action>> {
        Err(anyhow::anyhow!("Not supported via keymaps"))
    }
}

#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct OpenAbConfigFile {
    pub plugin_idx: usize,
    pub path_id: String, // "a" or "b"
}

impl gpui::Action for OpenAbConfigFile {
    fn boxed_clone(&self) -> Box<dyn gpui::Action> {
        Box::new(self.clone())
    }
    fn partial_eq(&self, other: &dyn gpui::Action) -> bool {
        other.as_any().downcast_ref::<Self>() == Some(self)
    }
    fn name(&self) -> &'static str {
        "OpenAbConfigFile"
    }
    fn name_for_type() -> &'static str {
        "OpenAbConfigFile"
    }
    fn build(_: serde_json::Value) -> anyhow::Result<Box<dyn gpui::Action>> {
        Err(anyhow::anyhow!("Not supported via keymaps"))
    }
}

// ============================================================================
// A/B Compare Sub-Rack Actions
// ============================================================================

macro_rules! ab_action {
    ($name:ident { $($field:ident : $ty:ty),* $(,)? }) => {
        #[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
        pub struct $name {
            $(pub $field: $ty),*
        }
        impl gpui::Action for $name {
            fn boxed_clone(&self) -> Box<dyn gpui::Action> { Box::new(self.clone()) }
            fn partial_eq(&self, other: &dyn gpui::Action) -> bool {
                other.as_any().downcast_ref::<Self>() == Some(self)
            }
            fn name(&self) -> &'static str { stringify!($name) }
            fn name_for_type() -> &'static str { stringify!($name) }
            fn build(_: serde_json::Value) -> anyhow::Result<Box<dyn gpui::Action>> {
                Err(anyhow::anyhow!("Not supported via keymaps"))
            }
        }
    };
}

// Add a plugin to an A/B path sub-rack. path: 0=A, 1=B.
ab_action!(ABPathAddPlugin { plugin_idx: usize, path: u8, plugin_type: String });
// Remove a plugin from an A/B path sub-rack.
ab_action!(ABPathRemovePlugin { plugin_idx: usize, path: u8, sub_idx: usize });
// Move a plugin within an A/B path sub-rack.
ab_action!(ABPathMovePlugin { plugin_idx: usize, path: u8, from: usize, to: usize });
// Toggle the "add plugin" dropdown for an A/B path.
ab_action!(ABPathToggleAddMenu { plugin_idx: usize, path: u8 });
