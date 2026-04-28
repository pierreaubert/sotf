//! Element wrapper that publishes its painted bounds to the dev
//! registry under a stable selector string.
//!
//! Usage:
//! ```ignore
//! use crate::app::dev_api::DevTrackExt;
//! div().id("foo").on_click(...).dev_track("library.play-button")
//! ```
//!
//! Compiled only when the `dev-api` feature is enabled — call sites
//! must guard the import or the call with `#[cfg(feature = "dev-api")]`.

use std::panic::Location;

use gpui::{
    App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement, LayoutId,
    Pixels, Window,
};

use super::registry;

pub struct DevTrack<E> {
    selector: String,
    inner: E,
}

impl<E: Element> Element for DevTrack<E> {
    type RequestLayoutState = E::RequestLayoutState;
    type PrepaintState = E::PrepaintState;

    fn id(&self) -> Option<ElementId> {
        self.inner.id()
    }

    fn source_location(&self) -> Option<&'static Location<'static>> {
        self.inner.source_location()
    }

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        self.inner.request_layout(id, inspector_id, window, cx)
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        registry::record(&self.selector, bounds);
        self.inner
            .prepaint(id, inspector_id, bounds, request_layout, window, cx)
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.inner
            .paint(id, inspector_id, bounds, request_layout, prepaint, window, cx)
    }
}

impl<E: Element> IntoElement for DevTrack<E> {
    type Element = Self;
    fn into_element(self) -> Self::Element {
        self
    }
}

pub trait DevTrackExt: IntoElement + Sized {
    fn dev_track(self, selector: impl Into<String>) -> DevTrack<Self::Element> {
        DevTrack {
            selector: selector.into(),
            inner: self.into_element(),
        }
    }
}

impl<I: IntoElement> DevTrackExt for I {}
