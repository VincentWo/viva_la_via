use std::{default, time::Duration};

use bevy::{log::{Level, LogPlugin}, prelude::*};

use geo::{
    Coord, Euclidean, InterpolatePoint, Length as _, LineInterpolatePoint, LineString, Point
};

use geo_bevy::line_string_to_mesh;


#[derive(Component, Debug)]
struct Position(f32);

#[derive(Component)]
struct OldPosition(f32);

#[derive(Component)]
struct Velocity(f32);

#[derive(Component)]
struct Blockabschnitt(LineString<f32>);

#[derive(Component)]
enum TrainCommand {
    Accelerate,
    Move,
    Break,
    Custom(f32),
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

fn update_train_command(mut query: Query<(&mut TrainCommand, &Position, &Blockabschnitt, &SpeedStats, &Velocity)>) {
    for (mut command, pos, block, speed_stats, velocity) in &mut query {
        let breaking_distance = velocity.0.powi(2) / (2.0 * speed_stats.brake_speed);
        let remaining_distance = block.0.length::<Euclidean>() - pos.0;
        if remaining_distance < breaking_distance {
            *command = TrainCommand::Break;
        } else {
            *command = TrainCommand::Accelerate;
        }
    }
}

fn update_speed(mut query: Query<(&mut Velocity, &SpeedStats ,&TrainCommand)>) {
    for (mut v, speed_stats, train_command) in &mut query {
        match train_command {
            TrainCommand::Accelerate => {
                v.0 = f32::min(v.0 + speed_stats.acceleration * ΔT, speed_stats.max_speed);
            }
            TrainCommand::Break => {
                v.0 = f32::max(v.0 - speed_stats.brake_speed * ΔT, 0.0);
            }
            TrainCommand::Move => {}
            TrainCommand::Custom(acc) => {
                v.0 = f32::min(v.0 + acc * ΔT, speed_stats.max_speed);
            }
        }
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
    mut query: Query<(&mut Transform, &OldPosition, &Position, &Blockabschnitt)>,
) {
    for (mut transform, old_pos, pos, block) in &mut query {
        let block_length = block.0.length::<Euclidean>();
        let old_pos_coords = block.0.line_interpolate_point(f32::min(old_pos.0 / block_length, 1.0)).unwrap_or(Point::new(0.0, 0.0));
        let pos_coords = block.0.line_interpolate_point(f32::min(pos.0 / block_length, 1.0)).unwrap_or(Point::new(0.0, 0.0));
        
        let interpolate = Euclidean::point_at_ratio_between(old_pos_coords, pos_coords, fixed_time.overstep_fraction());
        transform.translation = Into::<Vec2>::into(interpolate.x_y()).extend(0.0);
    }
}

fn create_strecke(
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut commands: Commands,
) {
    let line = LineString(vec![
        Coord{x: 0.0, y: 0.0},
        Coord{x: 20.0, y: 10.0},
        Coord{x: 40.0, y: 10.0},
        Coord{x: 60.0, y: 0.0},
        Coord{x: 60.0, y: 20.0},
        Coord{x: 40.0, y: 25.0},
        Coord{x: 20.0, y: 20.0},
    ]);

    let color = Color::hsl(30.0, 1.0, 0.57);
    let mesh = line_string_to_mesh(line.clone()).unwrap();

    

    commands.spawn((
        Mesh2d(meshes.add(mesh)),
        MeshMaterial2d(materials.add(color)),
        Blockabschnitt(line),
    ));
}

fn add_trains(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    strecke: Query<&Blockabschnitt>,
) {
    commands.spawn(Camera2d);

    let strecke = strecke.get_single().unwrap();
    let shape = meshes.add(Rectangle::new(30.0, 15.0));
    let color = Color::hsl(55.0, 1.0, 0.57);
    commands.spawn((
        Train,
        Mesh2d(shape),
        MeshMaterial2d(materials.add(color)),
        Transform::from_translation(Into::<Vec2>::into(strecke.0.0[0].x_y()).extend(0.0)),
        Position(0.0),
        OldPosition(0.0),
        Velocity(0.0),
        SpeedStats {
            acceleration: 0.1,
            brake_speed: 0.5,
            max_speed: 10.0 / 3.6,
        },
        Blockabschnitt(strecke.0.clone()),
        TrainCommand::Move,
    ));
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.build().set(LogPlugin {
            filter: "info,viva_la_via=trace".to_owned(),
            level: Level::TRACE,
            custom_layer: |_| None,
        }))
        // .insert_resource(Strecke {
        //     start: Vec2 {
        //         x: -600.0,
        //         y: -300.0,
        //     },
        //     parts: vec![Part::Straight { length: 1000.0 }],
        // })
        .insert_resource(Time::<Fixed>::from_duration(Duration::from_micros(500)))
        .add_systems(Startup, (create_strecke, add_trains).chain())
        .add_systems(FixedUpdate, (update_train_command, update_speed, update_positions).chain())
        .add_systems(Update, update_train_displays)
        .run();
}
