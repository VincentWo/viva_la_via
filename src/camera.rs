use bevy::{
    app::{App, Plugin, Startup, Update},
    core_pipeline::core_2d::Camera2d,
    ecs::{
        query::With,
        system::{Commands, Res, Single},
    },
    input::mouse::AccumulatedMouseScroll,
    math::{Vec3, Vec3Swizzles},
    render::camera::{Camera, OrthographicProjection},
    transform::components::{GlobalTransform, Transform},
    window::{PrimaryWindow, Window},
};

fn create_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

pub fn zoom_handler(
    mut camera_data: Single<(
        &Camera,
        &mut OrthographicProjection,
        &mut Transform,
        &GlobalTransform,
    )>,
    window: Single<&Window, With<PrimaryWindow>>,
    scroll: Res<AccumulatedMouseScroll>,
) {
    let (camera, projection, camera_transform, camera_global_transform) = &mut *camera_data;
    // We are changing the change exponentially, otherwise the zoom speed would depend on the
    // scale. In other words: this ensures that it takes the same amount of scroll to go from 1x to 2x
    // as from 2x to 4x. (All of this constants are guesswork and should probably be made configurable)
    let proposed_scale_change = 1.0 - 0.0006 * scroll.delta.y;
    let new_scale = (projection.scale * proposed_scale_change).clamp(0.025, 10.0);
    let scale_change = new_scale / projection.scale;
    projection.scale = new_scale;
    // Only changing the scale gives surprising results, since users usually want their cursor
    // to stay at the same game position. Hence we also move the camera
    if let Some(cursor_position) = window.cursor_position().map(|window_coordinates| {
        camera
            .viewport_to_world_2d(camera_global_transform, window_coordinates)
            .expect("coordinates have to be in bounds")
    }) {
        let origin = camera
            // 0.0 = center in ndc
            .ndc_to_world(
                camera_global_transform,
                Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
            )
            .unwrap()
            .xy();
        // The scale change increases the viewport distance of objects to the origin.
        // We calculate where the cursor would end up if we would not correct the
        // shift and then move the camera in the opposite direction.
        let new_cursor_position = scale_change * (cursor_position - origin) + origin;
        let shift = new_cursor_position - cursor_position;
        camera_transform.translation -= shift.extend(0.0);
    }
}

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, create_camera)
            .add_systems(Update, zoom_handler);
    }
}
