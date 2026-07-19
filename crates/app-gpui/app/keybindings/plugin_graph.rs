use crate::app::actions;
use gpui::KeyBinding;

/// Plugin Graph bindings live in their own context so graph editing wins over
/// the layered PlayerView navigation, playback, filter, and meter bindings.
pub(super) fn plugin_graph_bindings() -> Vec<KeyBinding> {
    vec![
        KeyBinding::new("tab", actions::GraphSelectNextNode, Some("PluginGraph")),
        KeyBinding::new(
            "shift-tab",
            actions::GraphSelectPreviousNode,
            Some("PluginGraph"),
        ),
        KeyBinding::new("]", actions::GraphSelectNextPluginType, Some("PluginGraph")),
        KeyBinding::new(
            "[",
            actions::GraphSelectPreviousPluginType,
            Some("PluginGraph"),
        ),
        KeyBinding::new("=", actions::GraphSelectNextPort, Some("PluginGraph")),
        KeyBinding::new("-", actions::GraphSelectPreviousPort, Some("PluginGraph")),
        KeyBinding::new("a", actions::GraphAddSelectedPlugin, Some("PluginGraph")),
        KeyBinding::new("enter", actions::GraphEditSelectedNode, Some("PluginGraph")),
        KeyBinding::new("e", actions::GraphEditSelectedNode, Some("PluginGraph")),
        KeyBinding::new("b", actions::GraphToggleSelectedBypass, Some("PluginGraph")),
        KeyBinding::new("c", actions::GraphConnectSelectedNode, Some("PluginGraph")),
        KeyBinding::new(
            "x",
            actions::GraphDisconnectSelectedNode,
            Some("PluginGraph"),
        ),
        KeyBinding::new(
            "delete",
            actions::GraphRemoveSelectedNode,
            Some("PluginGraph"),
        ),
        KeyBinding::new(
            "backspace",
            actions::GraphRemoveSelectedNode,
            Some("PluginGraph"),
        ),
        KeyBinding::new("left", actions::GraphMoveSelectedLeft, Some("PluginGraph")),
        KeyBinding::new(
            "right",
            actions::GraphMoveSelectedRight,
            Some("PluginGraph"),
        ),
        KeyBinding::new("up", actions::GraphMoveSelectedUp, Some("PluginGraph")),
        KeyBinding::new("down", actions::GraphMoveSelectedDown, Some("PluginGraph")),
        KeyBinding::new(
            "shift-left",
            actions::GraphMoveSelectedLeftLarge,
            Some("PluginGraph"),
        ),
        KeyBinding::new(
            "shift-right",
            actions::GraphMoveSelectedRightLarge,
            Some("PluginGraph"),
        ),
        KeyBinding::new(
            "shift-up",
            actions::GraphMoveSelectedUpLarge,
            Some("PluginGraph"),
        ),
        KeyBinding::new(
            "shift-down",
            actions::GraphMoveSelectedDownLarge,
            Some("PluginGraph"),
        ),
    ]
}
