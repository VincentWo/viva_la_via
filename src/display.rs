use bevy::prelude::*;

use geo::{Euclidean, InterpolatePoint, Length as _, LineInterpolatePoint, Point};

use itertools::Itertools;

use crate::infra::{BlockColors, Segment, SegmentTrain};

use crate::train_movement::{OldPosition, Position, TrainSchedule};

fn update_train_displays(
    fixed_time: Res<Time<Fixed>>,
    mut query: Query<(&mut Transform, &OldPosition, &Position, &TrainSchedule)>,
    blockabschnitt: Query<(&Segment, &SegmentTrain)>,
) {
    query
        .par_iter_mut()
        .for_each(|(mut transform, old_pos, pos, schedule)| {
            let (block, _) = blockabschnitt.get(schedule.current()).unwrap();
            let block_length = block.0.length::<Euclidean>();
            let old_pos_coords = block
                .0
                .line_interpolate_point(f32::min(old_pos.0 / block_length, 1.0))
                .unwrap_or(Point::new(0.0, 0.0));
            let pos_coords = block
                .0
                .line_interpolate_point(f32::min(pos.0 / block_length, 1.0))
                .unwrap_or(Point::new(0.0, 0.0));

            let interpolate = Euclidean::point_at_ratio_between(
                old_pos_coords,
                pos_coords,
                fixed_time.overstep_fraction(),
            );
            let interpolate_vec = Into::<Vec2>::into(interpolate.x_y()).extend(0.0);
            transform.translation = interpolate_vec;

            let angle =
                (pos_coords.y() - old_pos_coords.y()).atan2(pos_coords.x() - old_pos_coords.x());
            let old_rotation = transform.rotation;
            transform.rotation = old_rotation.lerp(Quat::from_rotation_z(angle), 0.15);
        });
}

fn update_block_display(
    mut query: Query<(&Segment, &SegmentTrain, &mut MeshMaterial2d<ColorMaterial>)>,
    blah: Res<BlockColors>,
) {
    query.iter_mut().for_each(|(_block, train, mut material)| {
        material.0 = if train.0.is_some() {
            blah.occupied.clone()
        } else {
            blah.free.clone()
        };
    });
}

pub struct TrainDisplayPlugin;

impl Plugin for TrainDisplayPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (update_train_displays, update_block_display));
    }
}
