use std::{default, time::Duration};

use bevy::{ecs::query, log::{Level, LogPlugin}, prelude::*};

use geo::{
    Coord, Euclidean, InterpolatePoint, Length as _, LineInterpolatePoint, LineString, Point, Scale, Translate
};

use geo_bevy::line_string_to_mesh;

use bevy_metrics_dashboard::{DashboardPlugin, DashboardWindow, RegistryPlugin, RenderMetricsPlugin, CoreMetricsPlugin };
use itertools::Itertools;
use metrics::{
    counter, describe_counter, describe_gauge, describe_histogram, gauge, histogram, Unit,
};
use bevy_egui::EguiPlugin;


#[derive(Component, Debug)]
struct Position(f32);

#[derive(Component)]
struct OldPosition(f32);

#[derive(Component)]
struct Velocity(f32);

#[derive(Component)]
struct Segment(LineString<f32>);


#[derive(Component)]
struct SegmentTrain(Option<Entity>);


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

#[derive(Resource)]
struct ConsecutiveLines(Vec<Entity>);

#[derive(Component)]
struct TrainSchedule {
    segments: Vec<Entity>,
    current_segment: usize,
}

#[derive(Event)]
struct LeavingSegment;

#[derive(Event)]
struct EnteringSegment(Entity);


impl TrainSchedule {
    fn next(&self) -> Option<Entity> {
        self.segments.get(self.current_segment + 1).copied()
    }

    fn previous(&self) -> Option<Entity> {
        self.segments.get(self.current_segment - 1).copied()
    }

    fn current(&self) -> Entity {
        self.segments[self.current_segment]
    }
    
}

const ΔT: f32 = 0.01;

fn update_train_command(
    mut query: Query<(&mut TrainCommand, &Position, &TrainSchedule, &SpeedStats, &Velocity)>,
    blockabschnitt: Query<(&Segment, &SegmentTrain)>,
) {
    query.par_iter_mut().for_each(|(mut command, pos, schedule, speed_stats, velocity): (Mut<'_, TrainCommand>, &Position, &TrainSchedule, &SpeedStats, &Velocity)| {
        let breaking_distance = velocity.0.powi(2) / (2.0 * speed_stats.brake_speed);
        let (block, _) = blockabschnitt.get(schedule.current()).unwrap();
        let next_block_free: bool = {
            match schedule.next() {
                Some(next_segment) => {
                    match blockabschnitt.get(next_segment) {
                        Ok((_, segment_train)) => {segment_train.0.is_none()}
                        Err(_) => false
                    }
                }
                None => false
            }
        };
        let remaining_distance = block.0.length::<Euclidean>() - pos.0;
        if next_block_free {
            *command = TrainCommand::Accelerate
        }
        else if remaining_distance < breaking_distance {
            *command = TrainCommand::Break;
        } else {
            *command = TrainCommand::Accelerate;
        }
    });
}

fn update_speed(mut query: Query<(&mut Velocity, &SpeedStats ,&TrainCommand)>, mut started: Local<bool>, key: Res<ButtonInput<KeyCode>>) {
    if key.just_pressed(KeyCode::Space) {
        *started = !*started;
    }
    if !*started {
        return;
    }
    query.par_iter_mut().for_each(|(mut v, speed_stats, train_command)| {
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
    });
}
fn update_positions(
    mut query: Query<(Entity, &mut Position, &mut OldPosition, &Velocity, &mut TrainSchedule )>,
    blockabschnitt: Query<(&Segment, &mut SegmentTrain)>,
    par_commands: ParallelCommands,
) {
    query.par_iter_mut().for_each(|(id, mut pos, mut old_pos, v, mut schedule)| {
        let mut next_pos = pos.0 + v.0 * ΔT;
        let mut old_pos_f: f32 = pos.0;
        let current_block = schedule.current();
        let (current_segment, _) = blockabschnitt.get(current_block).unwrap();
        let dist = current_segment.0.length::<Euclidean>();
        if next_pos >= dist{
            match schedule.next() {
                Some(next_block) => {
                    par_commands.command_scope(|mut commands| {
                        commands.trigger_targets(LeavingSegment, current_block);
                        commands.trigger_targets(EnteringSegment(id), next_block);
                    });
                    next_pos = pos.0 - dist;
                    schedule.current_segment += 1;
                    old_pos_f = 0.;
                }
                None => {
                    pos.0 = dist;
                }
            }
        }
        old_pos.0 = old_pos_f;
        pos.0 = next_pos;
        gauge!("train_speed").set(v.0);
        gauge!("train_position").set(pos.0);
    });
}

fn leaving_segment (
    trigger: Trigger<LeavingSegment>,
    mut query: Query<(&Segment, &mut SegmentTrain)>,
){
    let id = trigger.entity();
    let (_segment, mut segment_train) = query.get_mut(id).unwrap();
    segment_train.0 = None;
}

fn entering_segment (
    trigger: Trigger<EnteringSegment>,
    mut query: Query<(&Segment, &mut SegmentTrain)>,
){
    let id = trigger.entity();
    let (_segment, mut segment_train) = query.get_mut(id).unwrap();
    segment_train.0 = Some(trigger.0);
}

// fn draw_curve(curve: Res<Curve>, mut gizmos: Gizmos) {
//     gizmos.linestrip_2d(curve.0.samples(100).unwrap(), Color::hsv(10.0, 0.89, 0.46));
// }

fn update_train_displays(
    fixed_time: Res<Time<Fixed>>,
    mut query: Query<(&mut Transform, &OldPosition, &Position, &TrainSchedule)>,
    blockabschnitt: Query<(&Segment, &SegmentTrain)>
) {
    query.par_iter_mut().for_each(|(mut transform, old_pos, pos, schedule)| {
        let (block, _) = blockabschnitt.get(schedule.current()).unwrap();
        let block_length = block.0.length::<Euclidean>();
        let old_pos_coords = block.0.line_interpolate_point(f32::min(old_pos.0 / block_length, 1.0)).unwrap_or(Point::new(0.0, 0.0));
        let pos_coords = block.0.line_interpolate_point(f32::min(pos.0 / block_length, 1.0)).unwrap_or(Point::new(0.0, 0.0));
        
        let interpolate = Euclidean::point_at_ratio_between(old_pos_coords, pos_coords, fixed_time.overstep_fraction());
        let interpolate_vec = Into::<Vec2>::into(interpolate.x_y()).extend(0.0);
        transform.translation = interpolate_vec;
        
        let angle = (pos_coords.y() - old_pos_coords.y()).atan2(pos_coords.x() - old_pos_coords.x());
        let old_rotation = transform.rotation;
        transform.rotation = old_rotation.lerp(Quat::from_rotation_z(angle), 0.15);
    });
}

fn update_block_display(
    mut query: Query<(&Segment, &SegmentTrain, &mut MeshMaterial2d<ColorMaterial>)>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    query.iter_mut().for_each(|(_block, train, mut material)| {
        material.0 = materials.add(match train.0 {
            Some(_) => Color::hsl(0.0, 1.0, 0.57),
            None => Color::hsl(30.0, 1.0, 0.57),
        });
    });
}

fn create_strecke(
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut commands: Commands,
) {
    let mut line = LineString(vec![
        Coord{x: -60.0, y: 0.0},
        Coord{x: -50.0, y: 0.0},
        Coord{x: -40.0, y: 0.0},
        Coord{x: -30.0, y: -5.0},
        Coord{x: -20.0, y: -5.0},
        Coord{x: -10.0, y: 0.0},
        Coord{x: 0.0, y: 0.0},
        Coord{x: 10.0, y: 5.0},
        Coord{x: 20.0, y: 5.0},
        Coord{x: 30.0, y: 0.0},
        Coord{x: 40.0, y: 0.0},
        Coord{x: 50.0, y: 5.0},
        Coord{x: 60.0, y: 5.0},
    ]);
    line = line.clone().scale(10.0);
    let lines = line.coords().into_iter().tuple_windows().step_by(3).map(|(&a,&b,&c,&d)| LineString(vec![a,b,c,d])).collect::<Vec<LineString<f32>>>();

    let mut consecutive_lines: Vec<Entity> = Vec::new();
    let mut observer_leaving = Observer::new(leaving_segment);
    let mut observer_entering = Observer::new(entering_segment);

    for line in lines {
        let color = Color::hsl(30.0, 1.0, 0.57);
        let mesh = line_string_to_mesh(line.clone()).unwrap();
    
        consecutive_lines.push(commands.spawn((
            Mesh2d(meshes.add(mesh)),
            MeshMaterial2d(materials.add(color)),
            Segment(line.clone()),
            SegmentTrain(None),
        )).id());
    }
    commands.insert_resource(ConsecutiveLines(consecutive_lines.clone()));
    consecutive_lines.iter().for_each(|&e| {
        observer_entering.watch_entity(e);
        observer_leaving.watch_entity(e);
    });

    commands.spawn(observer_entering);
    commands.spawn(observer_leaving);
}

fn add_trains(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    strecke: Res<ConsecutiveLines>,
    mut blockabschnitt: Query<(&Segment, &mut SegmentTrain)>,
) {
    commands.spawn(Camera2d);

    let shape = meshes.add(Rectangle::new(20.0, 5.0));
    let color = Color::hsl(55.0, 1.0, 0.57);
    let (first_segment, mut first_segment_train) = blockabschnitt.get_mut(strecke.0[0]).unwrap();
    let train = commands.spawn((
        Train,
        Mesh2d(shape),
        MeshMaterial2d(materials.add(color)),
        Transform::from_translation(Into::<Vec2>::into(first_segment.0[0].x_y()).extend(0.0)),
        Position(0.0),
        OldPosition(0.0),
        Velocity(0.0),
        SpeedStats {
            acceleration: 0.1,
            brake_speed: 0.5,
            max_speed: 40.0 / 3.6,
        },
        TrainCommand::Move,
        TrainSchedule {
            segments: strecke.0.clone(),
            current_segment: 0,
        }
    )).id();
    first_segment_train.0 = Some(train);
}

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
        .insert_resource(Time::<Fixed>::from_duration(Duration::from_micros(500)))
        .add_systems(Startup, (create_dashboard, create_strecke, add_trains).chain())
        .add_systems(FixedUpdate, (update_train_command, update_speed, update_positions).chain())
        .add_systems(Update, (update_train_displays, update_block_display))
        .run();
}
