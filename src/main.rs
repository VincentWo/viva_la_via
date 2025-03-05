use std::time::Duration;

use bevy::{
    a11y::AccessibilityPlugin,
    animation::AnimationPlugin,
    app::{App, FixedUpdate, PanicHandlerPlugin, Startup, TerminalCtrlCHandlerPlugin, Update},
    asset::{AssetPlugin, Assets, RenderAssetUsages},
    color::Color,
    core::{FrameCountPlugin, TaskPoolPlugin, TypeRegistrationPlugin},
    core_pipeline::{CorePipelinePlugin, core_2d::Camera2d},
    dev_tools::DevToolsPlugin,
    diagnostic::DiagnosticsPlugin,
    ecs::{
        component::Component,
        schedule::IntoSystemConfigs as _,
        system::{Commands, Query, Res, ResMut},
    },
    gizmos::GizmoPlugin,
    gltf::GltfPlugin,
    input::InputPlugin,
    log::{Level, LogPlugin, debug},
    math::{FloatExt, Vec2, primitives::Rectangle},
    pbr::PbrPlugin,
    picking::DefaultPickingPlugins,
    render::{
        RenderPlugin,
        mesh::{Mesh, Mesh2d},
        pipelined_rendering::PipelinedRenderingPlugin,
        texture::ImagePlugin,
    },
    scene::ScenePlugin,
    sprite::{ColorMaterial, MeshMaterial2d, SpritePlugin},
    state::app::StatesPlugin,
    text::TextPlugin,
    time::{Fixed, Time, TimePlugin},
    transform::{TransformPlugin, components::Transform},
    ui::UiPlugin,
    window::{Window, WindowPlugin},
    winit::{WakeUp, WinitPlugin},
};

#[derive(Component, Debug)]
struct Position(f32);

#[derive(Component)]
struct OldPosition(f32);

#[derive(Component)]
struct Velocity(f32);

#[derive(bevy::ecs::system::Resource)]
struct Strecke {
    start: Vec2,
    parts: Vec<Part>,
}

enum Part {
    Straight { length: f32 },
}

#[derive(Component)]
struct SpeedStats {
    acceleration: f32,
    brake_speed: f32,
    max_speed: f32,
}

#[derive(Component)]
struct Train;

const ΔT: f32 = 0.01;

fn update_speed(mut query: Query<(&mut Velocity, &SpeedStats)>) {
    for (mut v, speed_stats) in &mut query {
        v.0 = f32::min(v.0 + speed_stats.acceleration * ΔT, speed_stats.max_speed);
    }
}
fn update_positions(mut query: Query<(&mut Position, &mut OldPosition, &Velocity)>) {
    for (mut pos, mut old_pos, v) in &mut query {
        old_pos.0 = pos.0;
        pos.0 += v.0 * ΔT;
    }
}

// fn draw_curve(curve: Res<Curve>, mut gizmos: Gizmos) {
//     gizmos.linestrip_2d(curve.0.samples(100).unwrap(), Color::hsv(10.0, 0.89, 0.46));
// }

fn update_train_displays(
    fixed_time: Res<Time<Fixed>>,
    mut query: Query<(&mut Transform, &OldPosition, &Position)>,
) {
    for (mut transform, old_pos, pos) in &mut query {
        let interpolate = old_pos.0.lerp(pos.0, fixed_time.overstep_fraction());
        debug!("{interpolate}");
        transform.translation.x = interpolate;
        // debug!(
        //     "Travelled with {}",
        //     // (transform.translation - new_translation).length() / time.delta_secs(),
        // );
        // transform.translation = new_translation;
    }
}

fn create_strecke(
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut commands: Commands,
    strecke: Res<Strecke>,
) {
    let vertices: Vec<_> = strecke
        .parts
        .iter()
        .scan(strecke.start, |last_pos, part| {
            last_pos.x += match part {
                Part::Straight { length } => length,
            };
            Some([last_pos.x, last_pos.y, 0.0])
        })
        .collect();
    let mesh = Mesh::new(
        bevy::render::mesh::PrimitiveTopology::LineStrip,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vertices);

    let mesh = meshes.add(mesh);

    let color = Color::hsl(30.0, 1.0, 0.57);

    commands.spawn((
        Mesh2d(mesh),
        MeshMaterial2d(materials.add(color)),
        Transform::from_xyz(0.0, 0.0, -1.0),
    ));
}

fn add_trains(
    mut commands: Commands,
    strecke: Res<Strecke>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn(Camera2d);

    let shape = meshes.add(Rectangle::new(30.0, 15.0));
    let color = Color::hsl(55.0, 1.0, 0.57);
    commands.spawn((
        Train,
        Mesh2d(shape),
        MeshMaterial2d(materials.add(color)),
        Transform::from_translation(strecke.start.extend(1.0)),
        Position(0.0),
        OldPosition(0.0),
        Velocity(0.0),
        SpeedStats {
            acceleration: 1.0,
            brake_speed: 1.5,
            max_speed: 160.0 / 3.6,
        },
    ));
}

fn main() {
    App::new()
        .add_plugins((
            PanicHandlerPlugin,
            LogPlugin {
                filter: "info,viva_la_via=trace".to_owned(),
                level: Level::TRACE,
                custom_layer: |_| None,
            },
            TaskPoolPlugin::default(),
            TypeRegistrationPlugin,
            FrameCountPlugin,
            TimePlugin,
            TransformPlugin,
            DiagnosticsPlugin,
            InputPlugin,
            WindowPlugin {
                primary_window: Some(Window::default()),
                exit_condition: bevy::window::ExitCondition::OnPrimaryClosed,
                close_when_requested: true,
            },
            AccessibilityPlugin,
            TerminalCtrlCHandlerPlugin,
            AssetPlugin::default(),
            ScenePlugin,
            WinitPlugin::<WakeUp>::default(),
        ))
        .add_plugins((
            RenderPlugin::default(),
            ImagePlugin::default(),
            PipelinedRenderingPlugin,
            CorePipelinePlugin,
            SpritePlugin::default(),
            TextPlugin,
            UiPlugin::default(),
            PbrPlugin::default(),
            GltfPlugin::default(),
            AnimationPlugin,
            GizmoPlugin,
            StatesPlugin,
            DevToolsPlugin,
            DefaultPickingPlugins,
        ))
        // .add_plugins(DefaultPlugins)
        .insert_resource(Strecke {
            start: Vec2 {
                x: -600.0,
                y: -300.0,
            },
            parts: vec![Part::Straight { length: 1000.0 }],
        })
        .insert_resource(Time::<Fixed>::from_duration(Duration::from_micros(500)))
        .add_systems(Startup, (create_strecke, add_trains))
        .add_systems(FixedUpdate, (update_speed, update_positions).chain())
        .add_systems(Update, update_train_displays)
        .run();
}
