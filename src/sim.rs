use bevy::prelude::*;
use bevy::tasks::prelude::*;
use bevy::core::FixedTimestep;

use crate::core::*;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[derive(SystemLabel)]
enum System {
  ComputeFriction,
  Integrate
}

pub fn simulation_stage() -> SystemStage {
  SystemStage::parallel()
    .with_run_criteria(FixedTimestep::step(DELTA_TIME))
    .with_system(compute_forces.before(System::ComputeFriction))
    .with_system(compute_friction
      .label(System::ComputeFriction)
      .before(System::Integrate))
    .with_system(integrate.label(System::Integrate))
    .with_system(wrap_around.after(System::Integrate))
    .with_system(update_shape.after(System::Integrate))
}

pub fn compute_forces(
  args: Res<ProgramArgs>,
  categories: Res<Categories>,
  sim_region: Res<SimRegion>,
  pool: Res<ComputeTaskPool>,
  mut particles_out: Query<(Entity, &Transform, &mut Acceleration, &CategoryId)>,
  particles_in: Query<(&Transform, &CategoryId)>
) {
  particles_out.par_for_each_mut(&pool, args.parallel_batch_size, |(entity, transform, mut acceleration, category)| {
    let neighbour_ids = sim_region.get_bucket_with_boundary(transform.translation.x, transform.translation.y);
    for nid in neighbour_ids {
      if nid == entity {
        continue;
      }
      let (other_transform, other_category) = particles_in.get(nid).unwrap();
      let delta = sim_region.get_corrected_position_delta(transform.translation, other_transform.translation);
      let distance_sq: f32 = delta.length_squared();
      if distance_sq > 1600.0 {
        return;
      }
      let distance = distance_sq.sqrt();
      let distance_unit_vector = delta / distance;
      if distance < 10.0 {
        let safety_margin_repulsion_force = (1000.0 - 100.0 * distance) * distance_unit_vector;
        acceleration.0 -= safety_margin_repulsion_force;
      } else {
        acceleration.0 += triangular_kernel(categories.0[other_category.0].force_coeffs[category.0], 30.0, 10.0, distance) * distance_unit_vector;
      }
    }
  });
}

pub fn compute_friction(mut particles: Query<(&Transform, &mut LastPosition, &mut Acceleration)>) {
  for (transform, mut last_pos, mut acceleration) in particles.iter_mut() {
    let velocity = (transform.translation - last_pos.0) / DELTA_TIME as f32;
    if velocity.length_squared() < VELOCITY_THRESHOLD {
      last_pos.0 = transform.translation;
    } else {
      let velocity_length = velocity.length();
      acceleration.0 -= 0.01 * velocity * velocity_length;
    }
  }
}

#[allow(dead_code)]
fn zigzag_kernel(magnitude: f32, middle: f32, width: f32, x: f32) -> f32 {
  magnitude * unit_zigzag((x - middle) / width)
}

#[allow(dead_code)]
fn unit_zigzag(x: f32) -> f32 {
  triangular_kernel(-1.0, -1.0, 2.0, 3.0 * x) + triangular_kernel(1.0, 1.0, 2.0, 3.0 * x)
}

fn triangular_kernel(magnitude: f32, middle: f32, width: f32, x: f32) -> f32 {
  magnitude * unit_triangle((x - middle) / width)
}

fn unit_triangle(x: f32) -> f32 {
  (1.0 - x.abs()).max(0.0)
}

pub fn integrate(mut query: Query<(&mut Acceleration, &mut Transform, &mut LastPosition)>) {
  let dt_sq = (DELTA_TIME * DELTA_TIME) as f32;
  for (mut acceleration, mut transform, mut last_pos) in query.iter_mut() {
    let new_pos =
        2.0 * transform.translation - last_pos.0 + acceleration.0 * dt_sq;
    acceleration.0 = Vec3::ZERO;
    last_pos.0 = transform.translation;
    transform.translation = new_pos;
  }
}

pub fn wrap_around(
  mut sim_region: ResMut<SimRegion>,
  mut query: Query<(Entity, &mut Transform, &mut LastPosition)>
) {
  for (entity, mut transform, mut last_pos) in query.iter_mut() {
    let x_old = last_pos.0.x;
    let y_old = last_pos.0.y;
    let adjustment = sim_region.get_wrap_around_adjustment(transform.translation);
    transform.translation += adjustment;
    last_pos.0 += adjustment;
    let x_new = transform.translation.x;
    let y_new = transform.translation.y;
    sim_region.move_entity(entity, x_old, y_old, x_new, y_new);
  }
}

pub fn update_shape(mut query: Query<(&mut Transform, &LastPosition)>) {
  for (mut transform, last_pos) in query.iter_mut() {
    let velocity = (transform.translation - last_pos.0).truncate();
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
  let angle = velocity.angle_between(Vec2::new(1.0, 0.0));
  Quat::from_rotation_z(-angle)
}