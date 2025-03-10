use std::time::Duration;

use bevy::{log::tracing_subscriber::field::debug, prelude::*, sprite::Anchor};

use geo::{Euclidean, Length as _};

use itertools::Itertools;
use metrics::{describe_gauge, gauge, Unit};

use crate::infra::{
    ConsecutiveLines, EnteringSegment, LeavingSegment, Segment, SegmentTrain, create_strecke,
};

const DELTA_T: f32 = 1.;

#[derive(Resource, Reflect)]
pub struct RealTime(pub Duration);

#[derive(Component, Debug, Default, Reflect)]
pub struct Position(pub f32);

#[derive(Component, Default, Reflect)]
pub struct OldPosition(pub f32);

#[derive(Component, Default, Reflect)]
pub struct Velocity(pub f32);

#[derive(Component, Default, Reflect)]
pub enum TrainCommand {
    Accelerate,
    #[default]
    Move,
    Break,
    Custom(f32),
}

impl TrainCommand {
    pub fn acceleration(&self, speed_stats: &SpeedStats) -> f32 {
        match self {
            TrainCommand::Accelerate => speed_stats.acceleration,
            TrainCommand::Break => -speed_stats.brake_speed,
            TrainCommand::Move => 0.0,
            TrainCommand::Custom(acc) => *acc,
        }
    }
}

#[derive(Component)]
pub struct SpeedStats {
    acceleration: f32,
    brake_speed: f32,
    max_speed: f32,
}

#[derive(Component)]
#[require(TrainCommand, Position, OldPosition, Velocity)]
pub struct Train;

#[derive(Component, Reflect)]
pub struct TrainSchedule {
    segments: Vec<Entity>,
    current_segment: usize,
}

impl TrainSchedule {
    pub fn next(&self) -> Option<Entity> {
        self.segments.get(self.current_segment + 1).copied()
    }

    pub fn previous(&self) -> Option<Entity> {
        self.segments.get(self.current_segment - 1).copied()
    }

    pub fn current(&self) -> Entity {
        self.segments[self.current_segment]
    }
}

fn update_train_command(
    mut query: Query<(
        &mut TrainCommand,
        &Position,
        &TrainSchedule,
        &SpeedStats,
        &Velocity,
        &Name,
    )>,
    blockabschnitt: Query<(&Segment, &SegmentTrain)>,
    mut started: Local<bool>,
    key: Res<ButtonInput<KeyCode>>,
) {
    if key.just_pressed(KeyCode::Space) {
        *started = true;
    }
    if !*started {
        return;
    }

    query.iter_mut().for_each(
        |(mut command, pos, schedule, speed_stats, velocity, name): (
            Mut<'_, TrainCommand>,
            &Position,
            &TrainSchedule,
            &SpeedStats,
            &Velocity,
            &Name,
        )| {
            let rel_pos = velocity.0 * DELTA_T;
            let rel_speed = command.acceleration(speed_stats) * DELTA_T;
            let breaking_distance =
                (velocity.0 + rel_speed).powi(2) / (2.0 * speed_stats.brake_speed);

            let (block, _) = blockabschnitt.get(schedule.current()).unwrap();
            let next_block_free: bool = {
                match schedule.next() {
                    Some(next_segment) => match blockabschnitt.get(next_segment) {
                        Ok((_, segment_train)) => segment_train.0.is_none(),
                        Err(_) => false,
                    },
                    None => false,
                }
            };
            let remaining_distance = block.0.length::<Euclidean>() - pos.0;
            gauge!(format!("{}_train_speed", name)).set(velocity.0);
            gauge!(format!("{}_train_position", name)).set(pos.0);
            gauge!(format!("{}_remaining_distance", name)).set(remaining_distance);

            if next_block_free {
                *command = TrainCommand::Accelerate
            } else if remaining_distance - rel_pos - rel_speed * DELTA_T <= breaking_distance + 10.
                || remaining_distance <= 10.
            {
                *command = TrainCommand::Break;
            } else {
                *command = TrainCommand::Accelerate;
            }
        },
    );
}

fn update_speed(
    mut query: Query<(&mut Velocity, &SpeedStats, &TrainCommand)>,
) {
    query
        .par_iter_mut()
        .for_each(|(mut v, speed_stats, train_command)| match train_command {
            TrainCommand::Accelerate => {
                v.0 = f32::min(
                    v.0 + speed_stats.acceleration * DELTA_T,
                    speed_stats.max_speed,
                );
            }
            TrainCommand::Break => {
                v.0 = f32::max(v.0 - speed_stats.brake_speed * DELTA_T, 0.0);
            }
            TrainCommand::Move => {}
            TrainCommand::Custom(acc) => {
                v.0 = f32::min(v.0 + acc * DELTA_T, speed_stats.max_speed);
            }
        });
}
fn update_positions(
    mut query: Query<(
        Entity,
        &mut Position,
        &mut OldPosition,
        &Velocity,
        &mut TrainSchedule,
    )>,
    blockabschnitt: Query<(&Segment, &mut SegmentTrain)>,
    par_commands: ParallelCommands,
) {
    query
        .par_iter_mut()
        .for_each(|(id, mut pos, mut old_pos, v, mut schedule)| {
            let mut next_pos = pos.0 + v.0 * DELTA_T;
            let mut old_pos_f: f32 = pos.0;
            let current_block = schedule.current();
            let (current_segment, _) = blockabschnitt.get(current_block).unwrap();
            let dist = current_segment.0.length::<Euclidean>();
            if next_pos >= dist {
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
        });
}

fn update_time(
    mut real_time: ResMut<RealTime>,
) {
    real_time.0 += Duration::from_millis((DELTA_T*1000.) as u64);
}

fn add_trains(
    mut commands: Commands,
    strecke: Res<ConsecutiveLines>,
    mut blockabschnitt: Query<(&Segment, &mut SegmentTrain)>,
) {

    let (first_segment, mut first_segment_train) = blockabschnitt.get_mut(strecke.0[0]).unwrap();
    let train1 = commands
        .spawn((
            Train,
            Name::new("Train 1"),
            Sprite {
                anchor: Anchor::Center,
                custom_size: Some(Vec2::new(20.0, 5.0)),
                color: Color::hsl(55.0, 1.0, 0.57),
                ..default()
            },
            Transform::from_translation(Into::<Vec2>::into(first_segment.0[0].x_y()).extend(0.0)),
            SpeedStats {
                acceleration: 0.8,
                brake_speed: 0.8,
                max_speed: 100.0 / 3.6,
            },
            TrainSchedule {
                segments: strecke.0.clone(),
                current_segment: 0,
            },
        ))
        .id();
    first_segment_train.0 = Some(train1);


    let (second_segment, mut second_segment_train) = blockabschnitt.get_mut(strecke.0[1]).unwrap();
    let train2 = commands
        .spawn((
            Train,
            Name::new("Train 2"),
            Sprite {
                anchor: Anchor::Center,
                custom_size: Some(Vec2::new(20.0, 5.0)),
                color: Color::hsl(55.0, 1.0, 0.57),
                ..default()
            },
            Transform::from_translation(Into::<Vec2>::into(second_segment.0[0].x_y()).extend(0.0)),
            SpeedStats {
                acceleration: 0.5,
                brake_speed: 0.5,
                max_speed: 30.0 / 3.6,
            },
            TrainSchedule {
                segments: strecke.0.clone(),
                current_segment: 1,
            },
        ))
        .id();
    second_segment_train.0 = Some(train2);

}

pub struct TrainMovementPlugin;

impl Plugin for TrainMovementPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Time::<Fixed>::from_duration(Duration::from_millis(50)))
            .add_systems(Startup, add_trains.after(create_strecke))
            .insert_resource(RealTime(Duration::from_secs(0)))
            .add_systems(
                FixedUpdate,
                (update_train_command, update_speed, update_positions, update_time).chain(),
            )
            .register_type::<Velocity>()
            .register_type::<Position>()
            .register_type::<OldPosition>();
    }
}
