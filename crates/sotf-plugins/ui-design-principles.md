I want to improve the usability of the UI for each audio plugin.

# principles

I want to do it from first principles:
1. each plugin has a set of features. Each feature has parameters and possible an on/off switch.
2. each parameter can be classified has important, usefull, less usefull and depending on its type can have different relationship.
3. a plugin is used via workflows and can be used for different purposes. Usually there is a configuration workflow (when you configure the plugin and want to set each parameter). The main usage is to touch some parameters only to optimise for the sound you want to have.
4. depending on the size of the UI you want to show more or less of the plugin
5. the UI should go left to right and top to bottom
6. the UI should be consistent between plugins and user should be able to find the same parameters in the same place:

+---------------------------------------------------------------------+
| menu ui | other menu if needed                 | menu preset | T S X|
+---------------------------------------------------------------------+
| config   |               main                 | diagnostic | output |
|          |                                    |            |        |
+---------------------------------------------------------------------+
| tab1 | tab2 | tabs3 ... |                                           |
+---------------------------------------------------------------------+
|      tabs content                                                   |
|                                                                     |
+---------------------------------------------------------------------+

7. if there is not enough space, diagnostic can become a tab. vice versa a tab can become a column if there is enough place.
8. main+config should remain visible at all time

# documentation

For each plugin, add a USAGE.md doc that
- describes the plugin, what it is used for
- explain each feature one by one
- add demos (that will be rendered in the HTML website later)
  - for ex the denoiser should have various demos that show the effect of each feature separately
- details of each feature with help on what each parameter is doing
- a set of classical presets that are used to optimize the plugin for each sound
  - for ex: compressor should have a few preset that are used for various sound

# UI design

Using the principles, the documentation of the plugin and the current UI in app-gpui, propose a new design per plugin that implement all the constraints. Render the UI graphically in Claude. The goal is to get agreement for each plugin first, and then implement them.

# UI implementation

The plugin will be implement in GPUI, SwiftUI and possibly for Android and windows natively. The descriptiont of the visual and interactions should be in a UI.md in each plugin which is the source of truth for the UI.



