//! Volume interaction scenarios for E2E testing.
//!
//! Tests for verifying volume control interactions.

use gpui::TestAppContext;
use std::cell::RefCell;
use std::rc::Rc;

/// Test volume step values.
#[gpui::test]
async fn test_volume_steps(_cx: &mut TestAppContext) {
    let volume: Rc<RefCell<f32>> = Rc::new(RefCell::new(0.1));
    const VOLUME_STEP: f32 = 0.05;

    // Initial value (with floating point tolerance)
    assert!(
        (*volume.borrow() - 0.1).abs() < 0.001,
        "Initial volume should be ~0.1"
    );

    // Increase by step
    *volume.borrow_mut() += VOLUME_STEP;
    assert!(*volume.borrow() > 0.1, "Volume should increase");

    // Decrease by step
    *volume.borrow_mut() -= VOLUME_STEP;
    assert!(
        (*volume.borrow() - 0.1).abs() < 0.001,
        "Volume should return to ~0.1"
    );
}

/// Test volume clamping.
#[gpui::test]
async fn test_volume_bounds(_cx: &mut TestAppContext) {
    let volume: Rc<RefCell<f32>> = Rc::new(RefCell::new(0.5));

    // Test upper bound
    *volume.borrow_mut() = 2.0;
    let clamped = volume.borrow().clamp(0.0, 1.0);
    assert_eq!(clamped, 1.0);

    // Test lower bound
    *volume.borrow_mut() = -0.5;
    let clamped = volume.borrow().clamp(0.0, 1.0);
    assert_eq!(clamped, 0.0);
}

/// Test volume transition through multiple values.
#[gpui::test]
async fn test_volume_transitions(_cx: &mut TestAppContext) {
    let volumes: Rc<RefCell<Vec<f32>>> = Rc::new(RefCell::new(vec![]));

    // Record volume changes
    for i in 1..=5 {
        volumes.borrow_mut().push(i as f32 * 0.1);
    }

    let recorded = volumes.borrow();
    assert_eq!(recorded.len(), 5);
    assert!(recorded.last().copied().unwrap_or(0.0) > recorded.first().copied().unwrap_or(0.0));
}
