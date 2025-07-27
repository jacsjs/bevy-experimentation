//! This example demonstrates how to use the `Camera::viewport_to_world` method.

use std::{f32::consts::{FRAC_PI_2, PI}, ops::Range};

use bevy::{input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll}, prelude::*, render::camera::ScalingMode, window::CursorGrabMode};

use crate::camera_controller::{CameraController, CameraControllerPlugin};

mod camera_controller;

#[derive(Component)]
struct Ground;

#[derive(Debug, Resource)]
struct CameraSettings {
    pub orbit_distance: f32,
    pub pitch_speed: f32,
    pub yaw_speed: f32,
    pub should_focus_at: Vec3,
    /// The height of the viewport in world units when the orthographic camera's scale is 1
    pub orthographic_viewport_height: f32,
    /// Clamp the orthographic camera's scale to this range
    pub orthographic_zoom_range: Range<f32>,
    /// Multiply mouse wheel inputs by this factor when using the orthographic camera
    pub orthographic_zoom_speed: f32,
    /// Clamp perspective camera's field of view to this range
    pub perspective_zoom_range: Range<f32>,
    /// Multiply mouse wheel inputs by this factor when using the perspective camera
    pub perspective_zoom_speed: f32,
}

impl Default for CameraSettings {
    fn default() -> Self {
        Self {
            orbit_distance: 20.0,
            pitch_speed: 0.01,
            yaw_speed: 0.01,
            should_focus_at: Vec3::ZERO,
            orthographic_viewport_height: 5.,
            // In orthographic projections, we specify camera scale relative to a default value of 1,
            // in which one unit in world space corresponds to one pixel.
            orthographic_zoom_range: 0.1..10.0,
            // This value was hand-tuned to ensure that zooming in and out feels smooth but not slow.
            orthographic_zoom_speed: 0.2,
            // Perspective projections use field of view, expressed in radians. We would
            // normally not set it to more than π, which represents a 180° FOV.
            perspective_zoom_range: (PI/ 20.)..(PI - 0.2),
            // Changes in FOV are much more noticeable due to its limited range in radians
            perspective_zoom_speed: 0.05,
        }
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .init_resource::<CameraSettings>()
        .add_systems(Startup, setup)
        .add_systems(Update,(orbit, zoom, draw_cursor)) 
        .run();
}

// This system grabs the mouse when the left mouse button is pressed
// and releases it when the escape key is pressed
fn grab_mouse(
    mut window: Single<&mut Window>,
    mouse: Res<ButtonInput<MouseButton>>,
) {
}

fn draw_cursor(
    query: Single<(&Camera, &mut Transform, &GlobalTransform), With<Camera3d>>,
    mut camera_settings: ResMut<CameraSettings>,
    ground: Single<&GlobalTransform, With<Ground>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    mut window: Single<&mut Window>,
    mut gizmos: Gizmos,
    time: Res<Time>,
) {
    let (camera, mut camera_transform, global_transform) = query.into_inner();

    let Some(cursor_position) = window.cursor_position() else {
        return;
    };

    // Calculate a ray pointing from the camera into the world based on the cursor's position.
    let Ok(ray) = camera.viewport_to_world(global_transform, cursor_position) else {
        return;
    };

    // Calculate if and where the ray is hitting the ground plane.
    let Some(distance) =
        ray.intersect_plane(ground.translation(), InfinitePlane3d::new(ground.up()))
    else {
        return;
    };
    let point = ray.get_point(distance);
    if mouse_buttons.pressed(MouseButton::Left) {
        // Calculate a ray pointing from the camera into the world based on the cursor's position.
        let Ok(ray2) = camera.viewport_to_world(global_transform, cursor_position + mouse_motion.delta) else {
            return;
        };

        // Calculate if and where the ray is hitting the ground plane.
        let Some(distance2) =
            ray2.intersect_plane(ground.translation(), InfinitePlane3d::new(ground.up()))
        else {
            return;
        };
        // calculate the camera motion based on the difference between where the camera is looking
        // and where it should be looking; the greater the distance, the faster the motion;
        // smooth out the camera movement using the frame time
        let camera_motion = ray2.get_point(distance2) - point;

        camera_settings.should_focus_at -= camera_motion;

        camera_transform.translation = camera_settings.should_focus_at - camera_transform.forward() * camera_settings.orbit_distance;
        println!("point: {}", camera_motion);
        println!("motion: {}", mouse_motion.delta);
    } else {
        // Draw a circle just above the ground plane at that position.
        gizmos.circle(
            Isometry3d::new(
                point + ground.up() * 0.01,
                Quat::from_rotation_arc(Vec3::Z, ground.up().as_vec3()),
            ),
            0.2,
            Color::WHITE,
        );
    }
}

fn orbit(
    mut camera: Single<&mut Transform, With<Camera3d>>,
    camera_settings: Res<CameraSettings>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
) {
    if mouse_buttons.pressed(MouseButton::Middle){
        let delta = mouse_motion.delta;

        let delta_pitch = delta.y * camera_settings.pitch_speed;
        let delta_yaw = delta.x * camera_settings.yaw_speed;

        let (yaw, pitch, roll) = camera.rotation.to_euler(EulerRot::YXZ);

        // If the pitch was ±¹⁄₂ π, the camera would look straight up or down.
        // When the user wants to move the camera back to the horizon, which way should the camera face?
        // The camera has no way of knowing what direction was "forward" before landing in that extreme position,
        // so the direction picked will for all intents and purposes be arbitrary.
        // Another issue is that for mathematical reasons, the yaw will effectively be flipped when the pitch is at the extremes.
        // To not run into these issues, we clamp the pitch to a safe range.
        const PITCH_LIMIT: f32 = FRAC_PI_2 - 0.01;
        let pitch = (pitch + delta_pitch).clamp(-PITCH_LIMIT, PITCH_LIMIT);

        let yaw = yaw + delta_yaw;
        camera.rotation = Quat::from_euler(EulerRot::YXZ, yaw, pitch, roll);

        camera.translation = camera_settings.should_focus_at - 
            camera.forward() * camera_settings.orbit_distance;
    }
}

fn zoom(
    camera: Single<&mut Projection, With<Camera3d>>,
    camera_settings: Res<CameraSettings>,
    mouse_wheel_input: Res<AccumulatedMouseScroll>,
) {
    // Usually, you won't need to handle both types of projection,
    // but doing so makes for a more complete example.
    match *camera.into_inner() {
        Projection::Orthographic(ref mut orthographic) => {
            // We want scrolling up to zoom in, decreasing the scale, so we negate the delta.
            let delta_zoom = -mouse_wheel_input.delta.y * camera_settings.orthographic_zoom_speed;
            // When changing scales, logarithmic changes are more intuitive.
            // To get this effect, we add 1 to the delta, so that a delta of 0
            // results in no multiplicative effect, positive values result in a multiplicative increase,
            // and negative values result in multiplicative decreases.
            let multiplicative_zoom = 1. + delta_zoom;

            orthographic.scale = (orthographic.scale * multiplicative_zoom).clamp(
                camera_settings.orthographic_zoom_range.start,
                camera_settings.orthographic_zoom_range.end,
            );
        }
        Projection::Perspective(ref mut perspective) => {
            // We want scrolling up to zoom in, decreasing the scale, so we negate the delta.
            let delta_zoom = -mouse_wheel_input.delta.y * camera_settings.perspective_zoom_speed;

            // Adjust the field of view, but keep it within our stated range.
            perspective.fov = (perspective.fov + delta_zoom).clamp(
                camera_settings.perspective_zoom_range.start,
                camera_settings.perspective_zoom_range.end,
            );
        }
        _ => (),
    }
}


fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // plane
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(20., 20.))),
        MeshMaterial3d(materials.add(Color::srgb(0.3, 0.5, 0.6))),
        Ground,
    ));

    // light
    commands.spawn((
        DirectionalLight::default(),
        Transform::from_translation(Vec3::ONE).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(15.0, 5.0, 15.0).looking_at(Vec3::ZERO, Vec3::Y),
        CameraController::default(),
    ));

    // Gltf asset testing
    commands.spawn(SceneRoot(asset_server.load(
        GltfAssetLabel::Scene(0).from_asset("models/tutorial_1.gltf"),
    )));
}




