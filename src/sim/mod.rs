use bevy::app::{TaskPoolOptions, TaskPoolThreadAssignmentPolicy};
use bevy::ecs::batching::BatchingStrategy;
use bevy::math::Vec3Swizzles;
use bevy::prelude::*;
use bevy::tasks::available_parallelism;

mod components;
mod forces;
mod grid;

pub use components::*;
pub use forces::compute_forces;
pub use grid::{Neighbour, SpatialGrid};

use crate::config::*;

#[derive(Debug, Default, Clone, Eq, PartialEq, Hash, States)]
pub enum SimState {
  #[default]
  Running,
  Paused,
}

/// Kolejność etapów kroku symulacji w `FixedUpdate`. Wykonują się tylko w stanie
/// `SimState::Running`.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum SimSet {
  Forces,
  Friction,
  Integrate,
  Wrap,
  Shape,
}

/// Pule wątków dla `TaskPoolPlugin`: pula obliczeniowa na wszystkie rdzenie. Domyślnie Bevy
/// oddaje część rdzeni pulom IO i async (na 10 rdzeniach zostają 4 na obliczenia), a z nich
/// korzysta tylko `compute_forces`; pule IO i async zostają domyślne i prawie nie pracują.
pub fn task_pool_options() -> TaskPoolOptions {
  TaskPoolOptions {
    compute: TaskPoolThreadAssignmentPolicy {
      min_threads: available_parallelism(),
      ..TaskPoolOptions::default().compute
    },
    ..default()
  }
}

/// Podział `compute_forces` na partie: wiele małych partii na wątek, żeby wątki, które
/// skończą wcześniej (rdzenie wydajne, cząstki poza skupiskami), przejmowały resztę pracy.
/// Cząstka w skupisku ma kilkadziesiąt razy więcej par niż reszta.
const FORCES_BATCHING: BatchingStrategy = BatchingStrategy {
  batch_size_limits: 64..usize::MAX,
  batches_per_thread: 16,
};

pub struct SimPlugin;

impl Plugin for SimPlugin {
  fn build(&self, app: &mut App) {
    app
      .insert_resource(Time::<Fixed>::from_seconds(DELTA_TIME))
      .init_state::<SimState>()
      .configure_sets(
        FixedUpdate,
        (
          SimSet::Forces,
          SimSet::Friction,
          SimSet::Integrate,
          SimSet::Wrap,
          SimSet::Shape,
        )
          .chain()
          .run_if(in_state(SimState::Running)),
      )
      .add_systems(
        FixedUpdate,
        (
          compute_forces.in_set(SimSet::Forces),
          compute_friction.in_set(SimSet::Friction),
          integrate.in_set(SimSet::Integrate),
          wrap_around.in_set(SimSet::Wrap),
          update_shape.in_set(SimSet::Shape),
        ),
      );
  }
}

pub fn compute_friction(
  mut particles: Query<(&Transform, &mut LastPosition, &mut Acceleration)>,
) {
  for (transform, mut last_pos, mut acceleration) in particles.iter_mut() {
    let velocity = (transform.translation.xy() - last_pos.0) / DELTA_TIME as f32;
    if velocity.length_squared() < VELOCITY_THRESHOLD {
      last_pos.0 = transform.translation.xy();
    } else {
      let velocity_length = velocity.length();
      acceleration.0 -= FRICTION_COEFF * velocity * velocity_length;
    }
  }
}

pub fn integrate(
  mut query: Query<(&mut Acceleration, &mut Transform, &mut LastPosition)>,
) {
  let dt_sq = (DELTA_TIME * DELTA_TIME) as f32;
  for (mut acceleration, mut transform, mut last_pos) in query.iter_mut() {
    let new_pos = 2.0 * transform.translation.xy() - last_pos.0 + acceleration.0 * dt_sq;
    acceleration.0 = Vec2::ZERO;
    last_pos.0 = transform.translation.xy();
    transform.translation = new_pos.extend(0.0);
  }
}

pub fn wrap_around(
  mut grid: ResMut<SpatialGrid>,
  mut query: Query<(Entity, &mut Transform, &mut LastPosition)>,
) {
  for (entity, mut transform, mut last_pos) in query.iter_mut() {
    let x_old = last_pos.0.x;
    let y_old = last_pos.0.y;
    let adjustment = grid.get_wrap_around_adjustment(transform.translation.xy());
    transform.translation += adjustment.extend(0.0);
    last_pos.0 += adjustment;
    let x_new = transform.translation.x;
    let y_new = transform.translation.y;
    grid.move_entity(entity, x_old, y_old, x_new, y_new);
  }
}

pub fn update_shape(
  mut query: Query<(&mut Transform, &LastPosition)>,
) {
  for (mut transform, last_pos) in query.iter_mut() {
    let velocity = transform.translation.xy() - last_pos.0;
    let velocity_length_sq = velocity.length_squared();
    if velocity_length_sq > VELOCITY_THRESHOLD {
      transform.scale = scale_from_velocity(velocity_length_sq);
      transform.rotation = rotation_from_velocity(velocity);
    }
  }
}

fn scale_from_velocity(velocity_length_sq: f32) -> Vec3 {
  let coeff = (2.0 + velocity_length_sq).log2();
  Vec3::new(coeff, 0.75 / coeff + 0.25, 1.0)
}

fn rotation_from_velocity(velocity: Vec2) -> Quat {
  let angle = velocity.angle_to(Vec2::new(1.0, 0.0));
  Quat::from_rotation_z(-angle)
}
