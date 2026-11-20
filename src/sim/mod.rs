use bevy::math::Vec3Swizzles;
use bevy::prelude::*;
use bevy::utils::Parallel;

mod components;
mod grid;

pub use components::*;
pub use grid::SpatialGrid;

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

pub fn compute_forces(
  particle_spec: Res<ParticleSpec>,
  grid: Res<SpatialGrid>,
  mut particles_out: Query<(Entity, &Transform, &mut Acceleration, &InteractionId)>,
  particles_in: Query<(Entity, &Transform, &InteractionId)>,
) {
  let mut queue: Parallel<Vec<(Entity, Vec2)>> = Parallel::default();
  particles_out.par_iter_mut().for_each_init(
    || queue.borrow_local_mut(),
    |local_queue, (entity, transform, _, interaction)| {
      let neighbours =
        grid.get_entities_by_position(transform.translation.x, transform.translation.y);
      let acceleration = neighbours
        .into_iter()
        .map(|other_entity| particles_in.get(other_entity).unwrap())
        .fold(
          Vec2::ZERO,
          |mut acceleration, (other_entity, other_transform, other_interaction)| {
            if entity == other_entity {
              return acceleration;
            }
            let delta = grid.get_corrected_position_delta(
              transform.translation.xy(),
              other_transform.translation.xy(),
            );
            let distance_sq: f32 = delta.length_squared();
            if distance_sq > INTERACTION_RADIUS * INTERACTION_RADIUS {
              return acceleration;
            }
            let distance = distance_sq.sqrt();
            let distance_unit_vector = delta / distance;
            if distance < REPULSION_RADIUS {
              let safety_margin_repulsion_force =
                REPULSION_STRENGTH * (REPULSION_RADIUS - distance) * distance_unit_vector;
              acceleration -= safety_margin_repulsion_force;
            } else {
              acceleration += triangular_kernel(
                particle_spec.interactions[other_interaction.0].force_coeffs[interaction.0],
                KERNEL_MIDDLE,
                KERNEL_WIDTH,
                distance,
              ) * distance_unit_vector;
            }
            acceleration
          },
        );
      local_queue.push((entity, acceleration))
    },
  );

  for batch in queue.iter_mut() {
    for (entity, accel) in batch {
      particles_out.get_mut(*entity).unwrap().2 .0 += *accel;
    }
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
