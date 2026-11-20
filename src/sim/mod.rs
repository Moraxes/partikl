use bevy::app::{TaskPoolOptions, TaskPoolThreadAssignmentPolicy};
use bevy::ecs::batching::BatchingStrategy;
use bevy::math::Vec3Swizzles;
use bevy::prelude::*;
use bevy::tasks::available_parallelism;

mod components;
mod grid;

pub use components::*;
pub use grid::{Neighbour, SpatialGrid};

use crate::config::*;
use crate::spec::ParticleSpec;

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

/// Siły od sąsiadów z siatki. Pozycje i typy sąsiadów pochodzą z siatki, nie z ECS, więc
/// siatka musi być zsynchronizowana z `Transform` (robi to `wrap_around`).
pub fn compute_forces(
  particle_spec: Res<ParticleSpec>,
  grid: Res<SpatialGrid>,
  mut particles: Query<(Entity, &Transform, &mut Acceleration, &InteractionId)>,
) {
  // Współczynniki w płaskiej tablicy: wiersz typu cząstki `me` to siły, jakimi działają na
  // nią typy kolejnych sąsiadów (`coeffs[me * types + other]`).
  let types = particle_spec.interactions.len();
  let coeffs: Vec<f32> = (0..types)
    .flat_map(|me| {
      particle_spec
        .interactions
        .iter()
        .map(move |other| other.force_coeffs[me])
    })
    .collect();

  particles
    .par_iter_mut()
    .batching_strategy(FORCES_BATCHING)
    .for_each(|(entity, transform, mut acceleration, interaction)| {
      let position = transform.translation.xy();
      let row = &coeffs[interaction.0 * types..(interaction.0 + 1) * types];
      let mut sum = Vec2::ZERO;
      for cell in grid.get_neighbour_cells(grid.bucket_coords(position.x, position.y)) {
        for other in cell {
          if other.entity == entity {
            continue;
          }
          let delta = grid.get_corrected_position_delta(position, other.position);
          let distance_sq: f32 = delta.length_squared();
          if distance_sq > INTERACTION_RADIUS * INTERACTION_RADIUS {
            continue;
          }
          let distance = distance_sq.sqrt();
          let distance_unit_vector = delta / distance;
          if distance < REPULSION_RADIUS {
            sum -= REPULSION_STRENGTH * (REPULSION_RADIUS - distance) * distance_unit_vector;
          } else {
            sum += triangular_kernel(row[other.interaction.0], KERNEL_MIDDLE, KERNEL_WIDTH, distance)
              * distance_unit_vector;
          }
        }
      }
      acceleration.0 += sum;
    });
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

fn triangular_kernel(magnitude: f32, middle: f32, width: f32, x: f32) -> f32 {
  magnitude * unit_triangle((x - middle) / width)
}

fn unit_triangle(x: f32) -> f32 {
  (1.0 - x.abs()).max(0.0)
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
