# GPUI - Comprehensive UI Framework Documentation

**GPUI** is a hybrid immediate and retained mode, GPU-accelerated UI framework for Rust, designed to support a wide variety of applications. It powers the Zed code editor.

## Table of Contents

1. [Core Concepts](#core-concepts)
2. [Application Lifecycle](#application-lifecycle)
3. [State Management with Entities](#state-management-with-entities)
4. [Views and Rendering](#views-and-rendering)
5. [Element Composition and Styling](#element-composition-and-styling)
6. [Event Handling](#event-handling)
7. [Async Operations](#async-operations)
8. [Focus Management](#focus-management)
9. [UI Patterns](#ui-patterns)
10. [Best Practices](#best-practices)

---

## Core Concepts

### Context Types

GPUI provides several context types for different scenarios:

- **`App`**: The root context providing access to global state and entities. Lives only on the main thread (not `Send`)
- **`Context<T>`**: Context for updating an `Entity<T>`. Dereferences to `App`
- **`Window`**: Manages window state, focus, and actions. Always passed before `cx` parameter
- **`AsyncApp`**: Owned context for async operations that can be held across await points
- **`AsyncWindowContext`**: Window context for async operations

**Key Rule**: `App` and all contexts are NOT `Send` - they only exist on the main/UI thread.

### Threading Model

- **Foreground thread**: All UI rendering and entity updates occur on a single main thread
- **Background thread pool**: CPU-intensive work runs on background executor
- **Tasks**: Futures that start running immediately (don't require `.await` to start)

---

## Application Lifecycle

### Basic Application Setup

```rust
use gpui::*;

fn main() {
    Application::new().run(|cx: &mut App| {
        // Create window
        let bounds = Bounds::centered(None, size(px(800.), px(600.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| {
                // Create root view
                cx.new(|_| HelloWorld {
                    text: "World".into(),
                })
            },
        )
        .unwrap();

        cx.activate(true);
    });
}
```

**File reference**: `crates/gpui/examples/hello_world.rs`

---

## State Management with Entities

### What is an Entity?

An `Entity<T>` is a strong, typed reference to state managed by GPUI. It's effectively a handle into the `App::EntityMap`.

### Creating Entities

```rust
// Create an entity
let my_entity = cx.new(|cx| MyState {
    counter: 0,
    name: "Example".into(),
});

// Create with initialization
let picker = cx.new(|cx| Picker::uniform_list(delegate, window, cx));
```

**File reference**: `crates/file_finder/src/file_finder.rs:168-199`

### Reading Entity State

```rust
// Immutable read
let value = my_entity.read(cx);
println!("Counter: {}", value.counter);

// Read with closure
let result = my_entity.read_with(cx, |state, cx| {
    state.counter
});
```

### Updating Entity State

```rust
// Update with mutation
my_entity.update(cx, |state, cx| {
    state.counter += 1;
    cx.notify(); // Trigger re-render
});

// Update with window access
my_entity.update_in(cx, |state, window, cx| {
    state.counter += 1;
    window.refresh();
});
```

**File reference**: `crates/file_finder/src/file_finder.rs:197-199`

### Weak Entity References

Use `WeakEntity<T>` to avoid circular references and memory leaks:

```rust
pub struct FileFinderDelegate {
    file_finder: WeakEntity<FileFinder>,
    workspace: WeakEntity<Workspace>,
}

// Get weak reference
let weak = entity.downgrade();

// Upgrade when needed
if let Some(strong) = weak.upgrade() {
    strong.update(cx, |state, cx| {
        // Use state
    });
}
```

**File reference**: `crates/file_finder/src/file_finder.rs:828-829`

### Entity Methods Summary

```rust
// With thing: Entity<T>
thing.entity_id()           // Returns EntityId
thing.downgrade()           // Returns WeakEntity<T>
thing.read(cx)              // Returns &T
thing.read_with(cx, |t, cx| ...)   // Returns closure result
thing.update(cx, |t, cx| ...)      // Mutate, returns closure result
thing.update_in(cx, |t, w, cx| ...)  // Mutate with window
```

---

## Views and Rendering

### The Render Trait

A view is an `Entity<T>` where `T` implements `Render`. The `Render` trait converts state into an element tree.

```rust
struct HelloWorld {
    text: SharedString,
}

impl Render for HelloWorld {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_3()
            .bg(rgb(0x505050))
            .size(px(500.0))
            .justify_center()
            .items_center()
            .shadow_lg()
            .border_1()
            .border_color(rgb(0x0000ff))
            .text_xl()
            .text_color(rgb(0xffffff))
            .child(format!("Hello, {}!", &self.text))
    }
}
```

**File reference**: `crates/gpui/examples/hello_world.rs:5-25`

### RenderOnce for Immutable Components

For components that don't need mutable state, implement `RenderOnce`:

```rust
#[derive(IntoElement)]
pub struct Disclosure {
    id: ElementId,
    is_open: bool,
    opened_icon: IconName,
    closed_icon: IconName,
    disabled: bool,
    selected: bool,
}

impl RenderOnce for Disclosure {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        IconButton::new(
            self.id,
            match self.is_open {
                true => self.opened_icon,
                false => self.closed_icon,
            },
        )
        .shape(IconButtonShape::Square)
        .icon_color(Color::Muted)
        .icon_size(IconSize::Small)
        .disabled(self.disabled)
        .toggle_state(self.selected)
    }
}
```

**Key difference**: `RenderOnce` takes ownership of `self`, `Render` takes `&mut self`.

**File reference**: `crates/ui/src/components/disclosure.rs:85-103`

### Triggering Re-renders

```rust
impl MyView {
    fn increment(&mut self, _: &Increment, _window: &mut Window, cx: &mut Context<Self>) {
        self.counter += 1;
        cx.notify(); // Tells GPUI this view needs re-rendering
    }
}
```

---

## Element Composition and Styling

### The Styled Trait

All GPUI elements implement the `Styled` trait, providing a Tailwind-like API:

```rust
div()
    .flex()              // display: flex
    .flex_col()          // flex-direction: column
    .gap_3()             // gap: 0.75rem
    .bg(rgb(0x505050))   // background-color
    .size(px(500.0))     // width & height
    .justify_center()    // justify-content: center
    .items_center()      // align-items: center
    .shadow_lg()         // box-shadow
    .border_1()          // border-width: 1px
    .border_color(rgb(0x0000ff))
    .text_xl()           // font-size
    .text_color(rgb(0xffffff))
    .child("Content")
```

**File reference**: `crates/gpui/src/styled.rs:35-88`

### Conditional Styling

Use `.when()` and `.when_some()` for conditional attributes:

```rust
v_flex()
    .id("container")
    .w_full()
    .flex_1()
    .gap(DynamicSpacing::Base08.rems(cx))
    .when(self.footer.is_some(), |this| this.pb_4())
    .when_some(
        self.container_scroll_handler,
        |this, scroll_handle| {
            this.overflow_y_scroll()
                .track_scroll(&scroll_handle)
        },
    )
    .children(self.children)
```

**File reference**: `crates/ui/src/components/modal.rs:73-88`

### Common Styling Methods

```rust
// Layout
.flex()                  // Display flex
.grid()                  // Display grid
.flex_row() / .flex_col()
.gap(px) / .gap_1() through .gap_4()
.p(px) / .px_2() / .py_3()  // Padding
.m(px) / .mx_2() / .my_3()  // Margin

// Sizing
.w_full() / .h_full()
.size(px) / .size_full()
.min_w(px) / .max_w(px)

// Colors
.bg(color) / .text_color(color)
.border_color(color)

// Text
.text_sm() / .text_base() / .text_xl()
.text_ellipsis()

// Borders
.border_1() / .border_2()
.rounded(px) / .rounded_md()

// Effects
.shadow_sm() / .shadow_lg()
.opacity(f32)

// Overflow
.overflow_hidden()
.overflow_scroll()
.overflow_x_scroll() / .overflow_y_scroll()
```

### Adding Children

```rust
div()
    .child("Single child")
    .child(other_element())
    .children(vec![element1, element2])
```

---

## Event Handling

### Click Events

```rust
Button::new("button_id", "Click me!")
    .on_click(|event, window, cx| {
        println!("Button clicked!");
    });
```

**File reference**: `crates/ui/src/components/button/button.rs:32-34`

### Using cx.listener for View Updates

When you need to access the view's mutable state in an event handler, use `cx.listener`:

```rust
impl Render for MyView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .on_click(cx.listener(|this: &mut MyView, event, window, cx| {
                this.counter += 1;
                cx.notify();
            }))
    }
}
```

### Action Handlers

Actions are user-defined events that can be triggered by keyboard shortcuts or code:

```rust
// Define actions
actions!(my_namespace, [SelectFirst, SelectLast]);

// Register action handlers
v_flex()
    .on_action(cx.listener(ContextMenu::select_first))
    .on_action(cx.listener(ContextMenu::select_next))
    .on_action(cx.listener(ContextMenu::confirm))
```

**File reference**: `crates/ui/src/components/context_menu.rs:660-685`

### Action Implementation

```rust
impl ContextMenu {
    fn select_first(&mut self, _: &menu::SelectFirst, window: &mut Window, cx: &mut Context<Self>) {
        self.selected_index = Some(0);
        cx.notify();
    }
}
```

### Mouse Events

```rust
div()
    .on_hover(cx.listener(|this, hovered, window, cx| {
        this.is_hovered = *hovered;
        cx.notify();
    }))
    .on_mouse_down(MouseButton::Left, cx.listener(|this, event, window, cx| {
        // Handle mouse down
    }))
    .on_mouse_down_out(cx.listener(|this, event, window, cx| {
        // Handle click outside
        this.cancel(window, cx);
    }))
```

**File reference**: `crates/ui/src/components/context_menu.rs:676-682`

### Event Emission and Subscriptions

Define that your type can emit events:

```rust
impl EventEmitter<DismissEvent> for FileFinder {}
```

Emit events from within the view:

```rust
impl FileFinder {
    fn cancel(&mut self, _: &menu::Cancel, window: &mut Window, cx: &mut Context<Self>) {
        cx.emit(DismissEvent);
    }
}
```

**File reference**: `crates/file_finder/src/file_finder.rs:1593,1603`

Subscribe to events from other entities:

```rust
// Store subscription to keep it alive
struct MyView {
    _subscription: Subscription,
}

// Subscribe in constructor
let _subscription = cx.subscribe(&other_entity, |this, emitter, event, cx| {
    // Handle event
});
```

### Observing Entity Changes

React when an entity calls `cx.notify()`:

```rust
let _observation = cx.observe(&entity, |this, observed_entity, cx| {
    // React to changes
});
```

**File reference**: `crates/gpui/src/app/context.rs:63-81`

---

## Async Operations

### Spawning Background Tasks

Use `background_spawn` for CPU-intensive work:

```rust
cx.background_spawn(async move {
    // Expensive computation on thread pool
    let result = complex_calculation();
    result
})
```

**File reference**: `crates/file_finder/src/file_finder.rs:150-156`

### Spawning Foreground Tasks

Use `spawn` for async work that needs to access entities:

```rust
cx.spawn(async move |cx| {
    let data = fetch_data().await;

    cx.update(|cx| {
        // Update state with fetched data
    }).ok();
})
```

### Spawn with Entity Context

When spawning from `Context<T>`, you get a `WeakEntity<T>` handle:

```rust
impl MyView {
    fn load_data(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |weak_handle, mut cx| {
            let data = fetch_data().await;

            weak_handle.update(&mut cx, |this, cx| {
                this.data = Some(data);
                cx.notify();
            }).ok();
        })
        .detach();
    }
}
```

**File reference**: `crates/gpui/src/app/context.rs:236-245`

### Spawn with Window Access

Use `spawn_in` when you need window access in async code:

```rust
cx.spawn_in(window, async move |workspace, cx| {
    let items = fetch_items().await;

    workspace.update_in(cx, |workspace, window, cx| {
        workspace.items = items;
        window.refresh();
    }).ok();
})
```

### Task Management

Tasks must be awaited, detached, or stored to prevent cancellation:

```rust
// Await the task
let result = task.await;

// Detach to let it run independently
task.detach();

// Detach and log errors
task.detach_and_log_err(cx);

// Store to keep alive while struct exists
struct MyView {
    pending_task: Option<Task<()>>,
}
```

### Creating Ready Tasks

For tasks that immediately return a value:

```rust
let task = Task::ready(42);
```

---

## Focus Management

### Focus Handles

Focus handles track which UI element has keyboard focus:

```rust
struct MyView {
    focus_handle: FocusHandle,
}

impl MyView {
    fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
        }
    }
}
```

**File reference**: `crates/file_finder/src/file_finder.rs:89-90`

### Focusable Trait

Implement `Focusable` to make your view focusable:

```rust
impl Focusable for CommandPalette {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.picker.focus_handle(cx)
    }
}
```

**File reference**: `crates/command_palette/src/command_palette.rs:138-141`

### Focus Events

React to focus changes:

```rust
let focus_handle = cx.focus_handle();

// On blur subscription
let _on_blur = cx.on_blur(&focus_handle, window, |this, window, cx| {
    this.cancel(window, cx);
});

// On focus subscription
let _on_focus = cx.on_focus(&focus_handle, window, |this, window, cx| {
    this.on_focused(window, cx);
});
```

**File reference**: `crates/ui/src/components/context_menu.rs:232-237`

### Controlling Focus

```rust
// Focus an element
window.focus(&focus_handle);

// Check if focused
if window.focused() == Some(&focus_handle) {
    // This element has focus
}
```

---

## UI Patterns

### Pattern 1: Modal Views

Modals are floating UI elements that appear on top of the main content:

```rust
#[derive(IntoElement)]
pub struct Modal {
    id: ElementId,
    header: ModalHeader,
    children: SmallVec<[AnyElement; 2]>,
    footer: Option<ModalFooter>,
    container_id: ElementId,
    container_scroll_handler: Option<ScrollHandle>,
}

impl Modal {
    pub fn new(id: impl Into<SharedString>, scroll_handle: Option<ScrollHandle>) -> Self {
        let id = id.into();
        Self {
            id: ElementId::Name(id.clone()),
            header: ModalHeader::new(),
            children: SmallVec::new(),
            footer: None,
            container_id: ElementId::Name(format!("{}_container", id).into()),
            container_scroll_handler: scroll_handle,
        }
    }

    pub fn header(mut self, header: ModalHeader) -> Self {
        self.header = header;
        self
    }

    pub fn section(mut self, section: Section) -> Self {
        self.children.push(section.into_any_element());
        self
    }
}

impl RenderOnce for Modal {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        v_flex()
            .id(self.id.clone())
            .size_full()
            .flex_1()
            .overflow_hidden()
            .child(self.header)
            .child(
                v_flex()
                    .id(self.container_id)
                    .w_full()
                    .flex_1()
                    .when(self.footer.is_some(), |this| this.pb_4())
                    .when_some(self.container_scroll_handler, |this, handle| {
                        this.overflow_y_scroll().track_scroll(&handle)
                    })
                    .children(self.children),
            )
            .when_some(self.footer, |this, footer| this.child(footer))
    }
}
```

**File reference**: `crates/ui/src/components/modal.rs`

### Pattern 2: Picker Pattern

Pickers provide searchable, filterable lists:

```rust
pub struct Picker<D: PickerDelegate> {
    pub delegate: D,
    element_container: ElementContainer,
    head: Head,
    pending_update_matches: Option<PendingUpdateMatches>,
    confirm_on_update: Option<bool>,
    width: Option<Length>,
    max_height: Option<Length>,
}

pub trait PickerDelegate: Sized + 'static {
    type ListItem: IntoElement;

    fn match_count(&self) -> usize;
    fn selected_index(&self) -> usize;
    fn set_selected_index(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Picker<Self>>);
    fn update_matches(&mut self, query: String, window: &mut Window, cx: &mut Context<Picker<Self>>) -> Task<()>;
    fn render_match(&self, ix: usize, window: &mut Window, cx: &mut Context<Picker<Self>>) -> Option<Self::ListItem>;
    fn confirm(&mut self, secondary: bool, window: &mut Window, cx: &mut Context<Picker<Self>>);
}
```

Example implementation:

```rust
pub struct FileFinder {
    picker: Entity<Picker<FileFinderDelegate>>,
}

impl FileFinder {
    fn new(workspace: WeakEntity<Workspace>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let delegate = FileFinderDelegate::new(/* ... */);
        let picker = cx.new(|cx| Picker::uniform_list(delegate, window, cx));

        Self { picker }
    }
}
```

**File reference**: `crates/picker/src/picker.rs:60-75`, `crates/file_finder/src/file_finder.rs:87-91`

### Pattern 3: Panel Pattern

Panels are dockable UI components:

```rust
pub trait Panel: Render + EventEmitter<PanelEvent> + Focusable {
    fn panel_id(&self) -> &SharedString;
    fn position(&self, cx: &App) -> DockPosition;
    fn position_is_valid(&self, position: DockPosition) -> bool;
    fn set_position(&mut self, position: DockPosition, cx: &mut Context<Self>);
    fn size(&self, cx: &App) -> Pixels;
    fn set_size(&mut self, size: Option<Pixels>, cx: &mut Context<Self>);
    fn icon(&self, cx: &App) -> Option<IconName>;
    fn icon_tooltip(&self, _cx: &App) -> Option<&'static str> { None }
    fn toggle_action(&self) -> Box<dyn Action>;
}
```

Example implementation:

```rust
pub struct ProjectPanel {
    project: Entity<Project>,
    fs: Arc<dyn Fs>,
    focus_handle: FocusHandle,
    scroll_handle: UniformListScrollHandle,
    workspace: WeakEntity<Workspace>,
    width: Option<Pixels>,
}

impl Panel for ProjectPanel {
    fn panel_id(&self) -> &SharedString {
        &PROJECT_PANEL_ID
    }

    fn position(&self, cx: &App) -> DockPosition {
        DockPosition::Left
    }

    // ... other trait methods
}
```

**File reference**: `crates/panel/src/panel.rs:18-42`, `crates/project_panel/src/project_panel.rs:111-157`

### Pattern 4: Context Menus

Context menus are right-click menus or dropdown menus:

```rust
impl ContextMenu {
    pub fn build(
        window: &mut Window,
        cx: &mut App,
        f: impl FnOnce(Self, &mut Window, &mut Context<Self>) -> Self,
    ) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx, f))
    }
}

// Building a context menu
let menu = ContextMenu::build(window, cx, |menu, window, cx| {
    menu.entry("Copy", None, cx.listener(|this, window, cx| {
        this.copy(window, cx);
    }))
    .entry("Paste", None, cx.listener(|this, window, cx| {
        this.paste(window, cx);
    }))
    .separator()
    .entry("Delete", None, cx.listener(|this, window, cx| {
        this.delete(window, cx);
    }))
});
```

Context menu entries:

```rust
pub struct ContextMenuEntry {
    label: SharedString,
    icon: Option<IconName>,
    handler: Rc<dyn Fn(Option<&FocusHandle>, &mut Window, &mut App)>,
    action: Option<Box<dyn Action>>,
    disabled: bool,
}

impl ContextMenuEntry {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            icon: None,
            handler: Rc::new(|_, _, _| {}),
            action: None,
            disabled: false,
        }
    }

    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn handler(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.handler = Rc::new(move |_, window, cx| handler(window, cx));
        self
    }

    pub fn action(mut self, action: Box<dyn Action>) -> Self {
        self.action = Some(action);
        self
    }
}
```

**File reference**: `crates/ui/src/components/context_menu.rs:65-142,226-259`

### Pattern 5: Lists and Scrolling

#### UniformList (Same-Height Items)

For lists where all items have the same height:

```rust
uniform_list(
    "my_list",
    item_count,
    |range, window, cx| {
        range.map(|ix| {
            ListItem::new(ix)
                .child(format!("Item {}", ix))
        }).collect()
    }
)
.track_scroll(scroll_handle)
```

With scroll handle:

```rust
struct MyView {
    scroll_handle: UniformListScrollHandle,
    items: Vec<Item>,
}

impl Render for MyView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        uniform_list(
            "items",
            self.items.len(),
            |range, window, cx| {
                self.render_items(range, window, cx)
            }
        )
        .track_scroll(&self.scroll_handle)
    }
}

// Scroll to item
self.scroll_handle.scroll_to_item(index);
```

**File reference**: `crates/gpui/src/elements/uniform_list.rs:18-55`

#### List (Variable-Height Items)

For lists where items have different heights:

```rust
let list_state = ListState::new(
    item_count,
    ListAlignment::Top,
    px(1000.), // estimated item height
    |ix, window, cx| {
        self.render_item(ix, window, cx)
            .into_any_element()
    }
);

list(list_state, |ix, window, cx| {
    self.render_item(ix, window, cx)
})
```

**File reference**: `crates/gpui/src/elements/list.rs:23-34`

#### ListItem Component

Styled list items with consistent appearance:

```rust
ListItem::new("item_id")
    .indent_level(1)
    .selected(is_selected)
    .start_slot(Icon::new(IconName::File))
    .end_slot(Label::new("Badge"))
    .on_click(|event, window, cx| {
        // Handle click
    })
    .child(Label::new("Item label"))
```

**File reference**: `crates/ui/src/components/list/list_item.rs:17-78`

### Pattern 6: Component Builder Pattern

Many GPUI components use the builder pattern:

```rust
Button::new("btn_id", "Click Me")
    .icon(IconName::Check)
    .icon_position(IconPosition::Start)
    .style(ButtonStyle::Filled)
    .disabled(is_disabled)
    .tooltip("Click to confirm")
    .on_click(|event, window, cx| {
        // Handle click
    })
```

### Pattern 7: Stateful vs Stateless Components

**Stateful (Entity + Render)**:
- Use when component needs to maintain state across renders
- Use when component needs to handle async operations
- Use when component needs subscriptions/observations

```rust
struct Counter {
    count: usize,
    _subscription: Subscription,
}

impl Render for Counter {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().child(format!("Count: {}", self.count))
    }
}
```

**Stateless (RenderOnce + IntoElement)**:
- Use for pure presentation components
- Use for components that just transform props to elements
- Generally more efficient

```rust
#[derive(IntoElement)]
struct Badge {
    text: SharedString,
    color: Color,
}

impl RenderOnce for Badge {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .px_2()
            .py_1()
            .rounded_md()
            .bg(self.color)
            .child(self.text)
    }
}
```

---

## Best Practices

### 1. Entity Lifecycle

✅ **DO**: Use weak references to avoid cycles
```rust
struct Parent {
    child: Entity<Child>,
}

struct Child {
    parent: WeakEntity<Parent>, // Avoids cycle
}
```

❌ **DON'T**: Create circular strong references
```rust
struct Parent {
    child: Entity<Child>,
}

struct Child {
    parent: Entity<Parent>, // Memory leak!
}
```

### 2. Task Management

✅ **DO**: Store tasks that should live as long as the view
```rust
struct MyView {
    background_task: Option<Task<()>>,
}

impl MyView {
    fn start_work(&mut self, cx: &mut Context<Self>) {
        self.background_task = Some(cx.spawn(async move |handle, cx| {
            // Work here
        }));
    }
}
```

✅ **DO**: Detach fire-and-forget tasks
```rust
cx.spawn(async move |cx| {
    // One-off work
}).detach();
```

❌ **DON'T**: Drop tasks without awaiting or detaching
```rust
cx.spawn(async move |cx| {
    // This will be cancelled immediately!
}); // Dropped here
```

### 3. Async Context Usage

✅ **DO**: Check if weak handle is valid
```rust
cx.spawn(async move |weak_handle, mut cx| {
    let data = fetch_data().await;

    weak_handle.update(&mut cx, |this, cx| {
        this.data = Some(data);
        cx.notify();
    }).ok(); // Handle the Result
})
```

❌ **DON'T**: Unwrap async entity updates
```rust
weak_handle.update(&mut cx, |this, cx| {
    // ...
}).unwrap(); // May panic if entity was dropped!
```

### 4. Focus Management

✅ **DO**: Use focus handles for keyboard interaction
```rust
div()
    .track_focus(&self.focus_handle)
    .on_action(cx.listener(|this, action, window, cx| {
        // Handle keyboard action
    }))
```

### 5. Styling

✅ **DO**: Use semantic spacing
```rust
div()
    .gap(DynamicSpacing::Base08.rems(cx))
    .px(DynamicSpacing::Base12.rems(cx))
```

✅ **DO**: Use conditional styling
```rust
div()
    .when(is_active, |div| div.bg(colors::blue))
    .when_some(icon, |div, icon| div.child(Icon::new(icon)))
```

### 6. Event Handlers

✅ **DO**: Use `cx.listener` for view methods
```rust
Button::new("id", "Click")
    .on_click(cx.listener(MyView::handle_click))
```

✅ **DO**: Use closures for inline handlers
```rust
Button::new("id", "Click")
    .on_click(|event, window, cx| {
        println!("Clicked!");
    })
```

### 7. Subscriptions

✅ **DO**: Store subscriptions to keep them alive
```rust
struct MyView {
    _subscriptions: Vec<Subscription>,
}

impl MyView {
    fn new(other: &Entity<Other>, cx: &mut Context<Self>) -> Self {
        let sub = cx.subscribe(other, |this, other, event, cx| {
            // Handle event
        });

        Self {
            _subscriptions: vec![sub],
        }
    }
}
```

### 8. Notify Pattern

✅ **DO**: Call `cx.notify()` after state changes
```rust
fn update_state(&mut self, cx: &mut Context<Self>) {
    self.value = new_value;
    cx.notify(); // Triggers re-render
}
```

### 9. Error Handling

✅ **DO**: Log errors from ignored async operations
```rust
cx.spawn(async move |handle, cx| {
    // Work that might fail
})
.detach_and_log_err(cx);
```

### 10. Component Composition

✅ **DO**: Break complex UIs into smaller components
```rust
impl Render for ComplexView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .child(self.render_header(cx))
            .child(self.render_content(cx))
            .child(self.render_footer(cx))
    }
}

impl ComplexView {
    fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // Header implementation
    }
}
```

---

## Common Pitfalls

### 1. Updating Entity During Update

❌ **PANIC**: Trying to update an entity while it's already being updated
```rust
entity.update(cx, |state, cx| {
    entity.update(cx, |state2, cx| { // PANIC!
        // Nested update of same entity
    });
});
```

### 2. Using Outer Context in Closures

❌ **ERROR**: Using outer `cx` inside entity update closure
```rust
let outer_cx = cx;
entity.update(cx, |state, cx| {
    outer_cx.notify(); // Wrong! Use inner cx
    cx.notify(); // Correct
});
```

### 3. Forgetting to Notify

❌ **BUG**: State changes without `cx.notify()` won't trigger re-render
```rust
fn update(&mut self, cx: &mut Context<Self>) {
    self.value = new_value;
    // Missing cx.notify()! View won't re-render
}
```

### 4. Not Handling Async Results

❌ **SILENT FAILURE**: Ignoring Result from async entity updates
```rust
cx.spawn(async move |handle, cx| {
    handle.update(&mut cx, |this, cx| {
        // This might fail silently
    });
});

// Better:
handle.update(&mut cx, |this, cx| {
    // ...
}).ok(); // Or .log_err()
```

### 5. Blocking the Main Thread

❌ **FREEZE**: Doing heavy work on foreground thread
```rust
cx.spawn(async move |cx| {
    // This blocks the UI!
    heavy_computation();
});

// Better:
cx.background_spawn(async move {
    heavy_computation()
})
```

---

## Quick Reference

### Creating Views

```rust
// Stateful view
cx.new(|cx| MyView::new(cx))

// Open window with view
cx.open_window(WindowOptions::default(), |window, cx| {
    cx.new(|cx| MyView::new(cx))
})
```

### Reading/Updating Entities

```rust
let value = entity.read(cx).field;
entity.update(cx, |state, cx| {
    state.field = value;
    cx.notify();
});
```

### Async Operations

```rust
// Background
cx.background_spawn(async move { /* CPU work */ })

// Foreground
cx.spawn(async move |weak, cx| { /* UI work */ })

// With window
cx.spawn_in(window, async move |weak, cx| { /* UI work */ })
```

### Common Elements

```rust
// Container
div().child("content")

// Flexbox
h_flex().children(items)  // Horizontal
v_flex().children(items)  // Vertical

// Text
Label::new("text")

// Button
Button::new("id", "label").on_click(handler)

// List
uniform_list("id", count, render_fn)
```

### Event Handling

```rust
// Click
.on_click(cx.listener(|this, event, window, cx| {}))

// Action
.on_action(cx.listener(MyView::handle_action))

// Subscribe
cx.subscribe(&entity, |this, entity, event, cx| {})

// Observe
cx.observe(&entity, |this, entity, cx| {})
```

---

## File References for Deep Dive

- **Core Framework**: `crates/gpui/src/`
- **UI Components**: `crates/ui/src/components/`
- **Picker Pattern**: `crates/picker/src/picker.rs`
- **File Finder Example**: `crates/file_finder/src/file_finder.rs`
- **Command Palette Example**: `crates/command_palette/src/command_palette.rs`
- **Project Panel Example**: `crates/project_panel/src/project_panel.rs`
- **Context Menus**: `crates/ui/src/components/context_menu.rs`
- **Modal Views**: `crates/ui/src/components/modal.rs`
- **Lists**: `crates/gpui/src/elements/uniform_list.rs`, `crates/gpui/src/elements/list.rs`

---

This documentation should provide a solid foundation for implementing UIs with GPUI. Refer to the Zed codebase for more complex examples and patterns.
