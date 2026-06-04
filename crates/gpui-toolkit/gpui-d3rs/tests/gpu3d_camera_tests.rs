use d3rs::gpu3d::Camera3D;
use glam::Vec3;

#[test]
fn billboard_axes_are_orthonormal() {
    let camera = Camera3D::default();
    let (right, up) = camera.billboard_axes();
    let forward = camera.forward();

    assert!((right.length() - 1.0).abs() < 1e-5);
    assert!((up.length() - 1.0).abs() < 1e-5);
    assert!(right.dot(up).abs() < 1e-5);
    assert!(right.dot(forward).abs() < 1e-5);
    assert!(up.dot(forward).abs() < 1e-5);
}

#[test]
fn project_to_screen_rejects_points_behind_camera() {
    let camera = Camera3D::new()
        .with_position(Vec3::new(0.0, 0.0, 2.0))
        .with_target(Vec3::ZERO);

    assert!(camera.project_to_screen(Vec3::ZERO, 400.0, 300.0).is_some());
    assert!(
        camera
            .project_to_screen(Vec3::new(0.0, 0.0, 3.0), 400.0, 300.0)
            .is_none()
    );
}
