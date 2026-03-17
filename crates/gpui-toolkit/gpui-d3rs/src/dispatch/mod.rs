//! # d3-dispatch - Event Dispatcher
//!
//! This module provides an event dispatcher inspired by D3.js's d3-dispatch module.
//! It allows registering callbacks for named events and dispatching events with payloads.
//!
//! ## Key Features
//!
//! - **Type-safe events**: Support for typed event payloads
//! - **Listener management**: Register, unregister, and list listeners
//! - **Context support**: Pass context to callbacks for state access
//! - **Copy-free dispatch**: Efficient event dispatch without unnecessary copying
//!
//! ## Example
//!
//! ```rust
//! use d3rs::dispatch::{Dispatcher, Event};
//!
//! let mut dispatcher = Dispatcher::new();
//!
//! // Register a listener
//! let handle = dispatcher.on("update", |event: &Event| {
//!     println!("Update received: {:?}", event.data);
//! });
//!
//! // Dispatch an event
//! dispatcher.dispatch("update", Some(Box::new("hello".to_string())));
//!
//! // Remove listener
//! dispatcher.off(handle);
//! ```

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

static LISTENER_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

/// A unique identifier for a listener
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ListenerId(u64);

impl ListenerId {
    fn new() -> Self {
        Self(LISTENER_ID_COUNTER.fetch_add(1, Ordering::SeqCst))
    }
}

/// An event dispatched to listeners
#[derive(Debug)]
pub struct Event {
    pub type_: String,
    pub data: Option<Box<dyn std::any::Any + Send + Sync>>,
}

impl Event {
    /// Create a new event with type and optional data
    pub fn new(type_: &str, data: Option<Box<dyn std::any::Any + Send + Sync>>) -> Self {
        Self {
            type_: type_.to_string(),
            data,
        }
    }

    /// Create an event with typed data
    pub fn with_data<T: 'static + Send + Sync>(type_: &str, data: T) -> Self {
        Self {
            type_: type_.to_string(),
            data: Some(Box::new(data)),
        }
    }

    /// Try to extract typed data from the event
    pub fn data<T: 'static + Send + Sync>(&self) -> Option<&T> {
        self.data.as_ref().and_then(|d| d.downcast_ref::<T>())
    }
}

impl Clone for Event {
    fn clone(&self) -> Self {
        Self {
            type_: self.type_.clone(),
            // Box<dyn Any> doesn't implement Clone, so we can only clone if data is None
            // This is a known limitation - for shared data, users should use Arc
            data: None,
        }
    }
}

/// A listener callback
type ListenerFn = Box<dyn FnMut(&Event) + Send + Sync>;

/// Internal listener storage
struct Listener {
    id: ListenerId,
    type_: String,
    callback: ListenerFn,
    #[allow(dead_code)]
    once: bool,
}

impl std::fmt::Debug for Listener {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Listener")
            .field("id", &self.id)
            .field("type_", &self.type_)
            .field("once", &self.once)
            .finish()
    }
}

/// Event dispatcher supporting named events with typed payloads
#[derive(Debug, Default)]
pub struct Dispatcher {
    listeners: Vec<Listener>,
}

impl Dispatcher {
    /// Create a new dispatcher
    pub fn new() -> Self {
        Self {
            listeners: Vec::new(),
        }
    }

    /// Register a listener for an event type
    ///
    /// Returns a `ListenerId` that can be used to remove the listener.
    ///
    /// # Example
    ///
    /// ```rust
    /// use d3rs::dispatch::{Dispatcher, Event};
    ///
    /// let mut dispatcher = Dispatcher::new();
    /// let handle = dispatcher.on("click", |event: &Event| {
    ///     println!("Click event!");
    /// });
    /// ```
    pub fn on<F>(&mut self, type_: &str, callback: F) -> ListenerId
    where
        F: FnMut(&Event) + Send + Sync + 'static,
    {
        let id = ListenerId::new();
        self.listeners.push(Listener {
            id,
            type_: type_.to_string(),
            callback: Box::new(callback),
            once: false,
        });
        id
    }

    /// Register a one-time listener that will be removed after first invocation
    ///
    /// # Example
    ///
    /// ```rust
    /// use d3rs::dispatch::{Dispatcher, Event};
    ///
    /// let mut dispatcher = Dispatcher::new();
    /// dispatcher.once("init", |event: &Event| {
    ///     println!("Initialized once!");
    /// });
    /// ```
    pub fn once<F>(&mut self, type_: &str, callback: F) -> ListenerId
    where
        F: FnOnce(&Event) + Send + Sync + 'static,
    {
        let id = ListenerId::new();
        let callback: Arc<Mutex<Option<F>>> = Arc::new(Mutex::new(Some(callback)));
        let callback_clone = callback.clone();
        self.listeners.push(Listener {
            id,
            type_: type_.to_string(),
            callback: Box::new(move |event: &Event| {
                if let Ok(mut cb) = callback_clone.lock()
                    && let Some(f) = cb.take()
                {
                    f(event);
                }
            }),
            once: true,
        });
        id
    }

    /// Remove a listener by its ID
    ///
    /// # Example
    ///
    /// ```rust
    /// use d3rs::dispatch::{Dispatcher, Event};
    ///
    /// let mut dispatcher = Dispatcher::new();
    /// let handle = dispatcher.on("update", |_| {});
    /// dispatcher.off(handle);
    /// ```
    pub fn off(&mut self, id: ListenerId) {
        self.listeners.retain(|l| l.id != id);
    }

    /// Remove all listeners for a specific event type
    ///
    /// # Example
    ///
    /// ```rust
    /// use d3rs::dispatch::{Dispatcher, Event};
    ///
    /// let mut dispatcher = Dispatcher::new();
    /// dispatcher.on("update", |_| {});
    /// dispatcher.on("update", |_| {});
    /// dispatcher.off_all("update");
    /// ```
    pub fn off_all(&mut self, type_: &str) {
        self.listeners.retain(|l| l.type_ != type_);
    }

    /// Dispatch an event to all registered listeners for that type
    ///
    /// # Example
    ///
    /// ```rust
    /// use d3rs::dispatch::{Dispatcher, Event};
    ///
    /// let mut dispatcher = Dispatcher::new();
    /// dispatcher.on("update", |event: &Event| {
    ///     if let Some(count) = event.data::<i32>() {
    ///         println!("Update count: {}", count);
    ///     }
    /// });
    /// dispatcher.dispatch("update", Some(Box::new(42i32)));
    /// ```
    pub fn dispatch(&mut self, type_: &str, data: Option<Box<dyn std::any::Any + Send + Sync>>) {
        let event = Event::new(type_, data);

        // Collect IDs of "once" listeners to remove after iteration
        let mut to_remove: Vec<ListenerId> = Vec::new();

        // Call each listener
        for listener in &mut self.listeners {
            if listener.type_ == type_ {
                (listener.callback)(&event);
                if listener.once {
                    to_remove.push(listener.id);
                }
            }
        }

        // Remove "once" listeners after iteration
        for id in to_remove {
            self.listeners.retain(|l| l.id != id);
        }
    }

    /// Dispatch an event with typed data
    ///
    /// # Example
    ///
    /// ```rust
    /// use d3rs::dispatch::{Dispatcher, Event};
    ///
    /// let mut dispatcher = Dispatcher::new();
    /// dispatcher.on("click", |event: &Event| {
    ///     if let Some(pos) = event.data::<(f64, f64)>() {
    ///         println!("Click at ({}, {})", pos.0, pos.1);
    ///     }
    /// });
    /// dispatcher.dispatch_typed("click", (100.0, 200.0));
    /// ```
    pub fn dispatch_typed<T: 'static + Send + Sync>(&mut self, type_: &str, data: T) {
        self.dispatch(type_, Some(Box::new(data)));
    }

    /// Check if there are any listeners for a given type
    ///
    /// # Example
    ///
    /// ```rust
    /// use d3rs::dispatch::{Dispatcher, Event};
    ///
    /// let mut dispatcher = Dispatcher::new();
    /// assert!(!dispatcher.has_listeners("click"));
    /// dispatcher.on("click", |_| {});
    /// assert!(dispatcher.has_listeners("click"));
    /// ```
    pub fn has_listeners(&self, type_: &str) -> bool {
        self.listeners.iter().any(|l| l.type_ == type_)
    }

    /// Get the number of listeners for a specific type
    pub fn listener_count(&self, type_: &str) -> usize {
        self.listeners.iter().filter(|l| l.type_ == type_).count()
    }

    /// Get the total number of listeners
    pub fn total_listeners(&self) -> usize {
        self.listeners.len()
    }

    /// Get all event types that have listeners
    pub fn event_types(&self) -> Vec<String> {
        let mut types: Vec<String> = self.listeners.iter().map(|l| l.type_.clone()).collect();
        types.sort();
        types.dedup();
        types
    }

    /// Clear all listeners
    pub fn clear(&mut self) {
        self.listeners.clear();
    }
}

/// Convenience function to create a dispatcher
///
/// # Example
///
/// ```rust
/// use d3rs::dispatch::dispatcher;
///
/// let mut disp = dispatcher();
/// disp.on("test", |_| {});
/// ```
pub fn dispatcher() -> Dispatcher {
    Dispatcher::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_dispatcher_on() {
        let mut disp = Dispatcher::new();
        let called = Arc::new(AtomicUsize::new(0));
        let called_clone = called.clone();

        let _handle = disp.on("test", move |_: &Event| {
            called_clone.fetch_add(1, Ordering::SeqCst);
        });

        disp.dispatch("test", None);
        assert_eq!(called.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_dispatcher_off() {
        let mut disp = Dispatcher::new();
        let called = Arc::new(AtomicUsize::new(0));
        let called_clone = called.clone();

        let handle = disp.on("test", move |_: &Event| {
            called_clone.fetch_add(1, Ordering::SeqCst);
        });

        disp.dispatch("test", None);
        assert_eq!(called.load(Ordering::SeqCst), 1);

        disp.off(handle);
        disp.dispatch("test", None);
        assert_eq!(called.load(Ordering::SeqCst), 1); // Still 1, not 2
    }

    #[test]
    fn test_dispatcher_multiple_listeners() {
        let mut disp = Dispatcher::new();
        let counter1 = Arc::new(AtomicUsize::new(0));
        let counter1_clone = counter1.clone();
        let counter2 = Arc::new(AtomicUsize::new(0));
        let counter2_clone = counter2.clone();

        disp.on("test", move |_: &Event| {
            counter1_clone.fetch_add(1, Ordering::SeqCst);
        });
        disp.on("test", move |_: &Event| {
            counter2_clone.fetch_add(1, Ordering::SeqCst);
        });

        disp.dispatch("test", None);
        assert_eq!(counter1.load(Ordering::SeqCst), 1);
        assert_eq!(counter2.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_dispatcher_different_types() {
        let mut disp = Dispatcher::new();
        let called_a = Arc::new(AtomicUsize::new(0));
        let called_b = Arc::new(AtomicUsize::new(0));
        let called_a_clone = called_a.clone();
        let called_b_clone = called_b.clone();

        disp.on("type_a", move |_: &Event| {
            called_a_clone.fetch_add(1, Ordering::SeqCst);
        });
        disp.on("type_b", move |_: &Event| {
            called_b_clone.fetch_add(1, Ordering::SeqCst);
        });

        disp.dispatch("type_a", None);
        assert_eq!(called_a.load(Ordering::SeqCst), 1);
        assert_eq!(called_b.load(Ordering::SeqCst), 0);

        disp.dispatch("type_b", None);
        assert_eq!(called_a.load(Ordering::SeqCst), 1);
        assert_eq!(called_b.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_dispatcher_with_data() {
        let mut disp = Dispatcher::new();

        disp.on("update", |event: &Event| {
            let count: Option<&i32> = event.data();
            assert_eq!(count, Some(&42));
        });

        disp.dispatch("update", Some(Box::new(42i32)));
    }

    #[test]
    fn test_dispatcher_dispatch_typed() {
        let mut disp = Dispatcher::new();

        disp.on("click", |event: &Event| {
            let pos: Option<&(f64, f64)> = event.data();
            assert!(pos.is_some());
            assert_eq!(pos.unwrap(), &(100.0, 200.0));
        });

        disp.dispatch_typed("click", (100.0, 200.0));
    }

    #[test]
    fn test_dispatcher_has_listeners() {
        let mut disp = Dispatcher::new();

        assert!(!disp.has_listeners("test"));
        disp.on("test", |_: &Event| {});
        assert!(disp.has_listeners("test"));
    }

    #[test]
    fn test_dispatcher_listener_count() {
        let mut disp = Dispatcher::new();

        assert_eq!(disp.listener_count("test"), 0);
        disp.on("test", |_: &Event| {});
        assert_eq!(disp.listener_count("test"), 1);
        disp.on("test", |_: &Event| {});
        assert_eq!(disp.listener_count("test"), 2);
    }

    #[test]
    fn test_dispatcher_off_all() {
        let mut disp = Dispatcher::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();
        let counter_clone2 = counter_clone.clone();

        disp.on("test", move |_: &Event| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });
        disp.on("test", move |_: &Event| {
            counter_clone2.fetch_add(1, Ordering::SeqCst);
        });

        disp.dispatch("test", None);
        assert_eq!(counter.load(Ordering::SeqCst), 2);

        disp.off_all("test");
        disp.dispatch("test", None);
        assert_eq!(counter.load(Ordering::SeqCst), 2); // Still 2
        assert_eq!(disp.listener_count("test"), 0);
    }

    #[test]
    fn test_dispatcher_event_types() {
        let mut disp = Dispatcher::new();

        disp.on("a", |_: &Event| {});
        disp.on("b", |_: &Event| {});
        disp.on("a", |_: &Event| {}); // Duplicate

        let types = disp.event_types();
        assert_eq!(types.len(), 2);
        assert!(types.contains(&"a".to_string()));
        assert!(types.contains(&"b".to_string()));
    }

    #[test]
    fn test_dispatcher_clear() {
        let mut disp = Dispatcher::new();

        disp.on("a", |_: &Event| {});
        disp.on("b", |_: &Event| {});
        assert_eq!(disp.total_listeners(), 2);

        disp.clear();
        assert_eq!(disp.total_listeners(), 0);
    }

    #[test]
    fn test_dispatcher_once() {
        let mut disp = Dispatcher::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        disp.once("test", move |_: &Event| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });

        disp.dispatch("test", None);
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        disp.dispatch("test", None);
        assert_eq!(counter.load(Ordering::SeqCst), 1); // Still 1 - removed after first call
    }

    #[test]
    fn test_dispatcher_unique_ids() {
        let mut disp = Dispatcher::new();
        let handle1 = disp.on("a", |_: &Event| {});
        let handle2 = disp.on("b", |_: &Event| {});
        let handle3 = disp.on("a", |_: &Event| {});

        assert_ne!(handle1, handle2);
        assert_ne!(handle2, handle3);
        assert_ne!(handle1, handle3);
    }

    #[test]
    fn test_dispatcher_event_new() {
        let event = Event::new("click", None);
        assert_eq!(event.type_, "click");
        assert!(event.data.is_none());
    }

    #[test]
    fn test_dispatcher_event_with_data() {
        let event = Event::with_data("update", 42i32);
        assert_eq!(event.type_, "update");
        assert_eq!(event.data::<i32>(), Some(&42));
    }

    #[test]
    fn test_dispatcher_event_data_wrong_type() {
        let event = Event::with_data("test", 42i32);
        let result: Option<&String> = event.data();
        assert!(result.is_none());
    }

    #[test]
    fn test_dispatcher_default() {
        let disp: Dispatcher = Default::default();
        assert_eq!(disp.total_listeners(), 0);
    }

    #[test]
    fn test_dispatcher_function() {
        let mut disp = dispatcher();
        disp.on("test", |_: &Event| {});
        assert!(disp.has_listeners("test"));
    }

    #[test]
    fn test_dispatcher_complex_data() {
        #[derive(Debug, Clone, PartialEq)]
        struct ComplexData {
            name: String,
            value: i32,
            coords: (f64, f64),
        }

        let mut disp = Dispatcher::new();

        disp.on("complex", |event: &Event| {
            let data: Option<&ComplexData> = event.data();
            assert!(data.is_some());
            let d = data.unwrap();
            assert_eq!(d.name, "test");
            assert_eq!(d.value, 100);
            assert_eq!(d.coords, (1.0, 2.0));
        });

        let data = ComplexData {
            name: "test".to_string(),
            value: 100,
            coords: (1.0, 2.0),
        };
        disp.dispatch_typed("complex", data);
    }

    #[test]
    fn test_dispatcher_no_listeners_for_type() {
        let mut disp = Dispatcher::new();
        disp.on("a", |_: &Event| {});

        // Should not panic, just do nothing
        disp.dispatch("nonexistent", None);
    }

    #[test]
    fn test_dispatcher_send_sync() {
        fn check_send_sync<T: Send + Sync>() {}
        check_send_sync::<Dispatcher>();
        check_send_sync::<ListenerId>();
        check_send_sync::<Event>();
    }

    #[test]
    fn test_dispatcher_multiple_onces() {
        let mut disp = Dispatcher::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();
        let counter_clone2 = counter_clone.clone();

        disp.once("test", move |_: &Event| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });
        disp.once("test", move |_: &Event| {
            counter_clone2.fetch_add(1, Ordering::SeqCst);
        });

        disp.dispatch("test", None);
        assert_eq!(counter.load(Ordering::SeqCst), 2);

        disp.dispatch("test", None);
        assert_eq!(counter.load(Ordering::SeqCst), 2); // Both removed
        assert_eq!(disp.listener_count("test"), 0);
    }

    #[test]
    fn test_dispatcher_clone_event_with_none_data() {
        let event = Event::new("test", None);
        let cloned = event.clone();
        assert_eq!(cloned.type_, "test");
        assert!(cloned.data.is_none());
    }
}
