use bevy::{
    log::{Level, LogPlugin},
    prelude::*,
};

use bevy_egui::EguiPlugin;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_metrics_dashboard::{
    CoreMetricsPlugin, DashboardPlugin, DashboardWindow, RegistryPlugin, RenderMetricsPlugin,
};
use metrics::{Unit, describe_gauge};

mod infra;
use crate::infra::InfraPlugin;

mod train_movement;
use crate::train_movement::TrainMovementPlugin;

mod display;
use crate::display::TrainDisplayPlugin;

// fn draw_curve(curve: Res<Curve>, mut gizmos: Gizmos) {
//     gizmos.linestrip_2d(curve.0.samples(100).unwrap(), Color::hsv(10.0, 0.89, 0.46));
// }

fn create_dashboard(mut commands: Commands) {
    commands.spawn(DashboardWindow::new("Metrics Dashboard"));
}

fn describe_metrics() {
    describe_gauge!("train_speed", Unit::Count, "Speed of trains");
    describe_gauge!("train_position", Unit::Count, "Position of trains");
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.build().set(LogPlugin {
            filter: "info,viva_la_via=trace".to_owned(),
            level: Level::TRACE,
            custom_layer: |_| None,
        }))
        .add_plugins(EguiPlugin)
        .add_plugins(RegistryPlugin::default())
        .add_plugins(CoreMetricsPlugin)
        .add_plugins(RenderMetricsPlugin)
        .add_plugins(DashboardPlugin)
        .add_plugins(WorldInspectorPlugin::new())
        .add_plugins(InfraPlugin)
        .add_plugins(TrainMovementPlugin)
        .add_plugins(TrainDisplayPlugin)
        .add_systems(Startup, create_dashboard)
        .run();
}
