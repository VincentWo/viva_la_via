use bevy::{log::tracing_subscriber::field::debug, prelude::*};

use geo::{Coord, LineString, Scale};

use geo_bevy::line_string_to_mesh;

use itertools::Itertools;

#[derive(Resource, Reflect)]
pub struct ConsecutiveLines(pub Vec<Entity>);

#[derive(Event)]
pub struct LeavingSegment;

#[derive(Event)]
pub struct EnteringSegment(pub Entity);

#[derive(Component, Debug)]
pub struct Segment(pub LineString<f32>);

#[derive(Component, Reflect)]
pub struct SegmentTrain(pub Option<Entity>);

#[derive(Resource, Debug, Reflect)]
pub struct BlockColors {
    pub occupied: Handle<ColorMaterial>,
    pub free: Handle<ColorMaterial>,
}

impl FromWorld for BlockColors {
    fn from_world(world: &mut World) -> Self {
        let mut materials = world.get_resource_mut::<Assets<ColorMaterial>>().unwrap();
        BlockColors {
            occupied: materials.add(Color::hsl(0.0, 1.0, 0.57)),
            free: materials.add(Color::hsl(30.0, 1.0, 0.57)),
        }
    }
}

fn leaving_segment(
    trigger: Trigger<LeavingSegment>,
    mut query: Query<(&Segment, &mut SegmentTrain)>,
) {
    let id = trigger.entity();
    let (_segment, mut segment_train) = query.get_mut(id).unwrap();
    segment_train.0 = None;
}

fn entering_segment(
    trigger: Trigger<EnteringSegment>,
    mut query: Query<(&Segment, &mut SegmentTrain)>,
) {
    let id = trigger.entity();
    let (_segment, mut segment_train) = query.get_mut(id).unwrap();
    segment_train.0 = Some(trigger.0);
}

pub fn create_strecke(
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut commands: Commands,
) {
    let mut line = LineString(vec![
        Coord { x: -60.0, y: 0.0 },
        Coord { x: -50.0, y: 0.0 },
        Coord { x: -40.0, y: 0.0 },
        Coord { x: -30.0, y: -5.0 },
        Coord { x: -20.0, y: -5.0 },
        Coord { x: -10.0, y: 0.0 },
        Coord { x: 0.0, y: 0.0 },
        Coord { x: 10.0, y: 5.0 },
        Coord { x: 20.0, y: 5.0 },
        Coord { x: 30.0, y: 0.0 },
        Coord { x: 40.0, y: 0.0 },
        Coord { x: 50.0, y: 5.0 },
        Coord { x: 60.0, y: 5.0 },
    ]);
    line = line.clone().scale(10.0);
    let lines = line
        .coords()
        .into_iter()
        .tuple_windows()
        .step_by(3)
        .map(|(&a, &b, &c, &d)| LineString(vec![a, b, c, d]))
        .collect::<Vec<LineString<f32>>>();

    let mut consecutive_lines: Vec<Entity> = Vec::new();
    let mut observer_leaving = Observer::new(leaving_segment);
    let mut observer_entering = Observer::new(entering_segment);

    let color = Color::hsl(30.0, 1.0, 0.57);
    let color_material = materials.add(color);
    for line in lines {
        let mesh = line_string_to_mesh(line.clone()).unwrap();

        consecutive_lines.push(
            commands
                .spawn((
                    Mesh2d(meshes.add(mesh)),
                    MeshMaterial2d(color_material.clone()),
                    Segment(line.clone()),
                    SegmentTrain(None),
                ))
                .id(),
        );
    }

    commands.insert_resource(ConsecutiveLines(consecutive_lines.clone()));
    consecutive_lines.iter().for_each(|&e| {
        observer_entering.watch_entity(e);
        observer_leaving.watch_entity(e);
    });

    commands.spawn(observer_entering);
    commands.spawn(observer_leaving);
}

pub struct InfraPlugin;

impl Plugin for InfraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, create_strecke)
            .init_resource::<BlockColors>()
            .register_type::<BlockColors>()
            .register_type::<ConsecutiveLines>();
    }
}
