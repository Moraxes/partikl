use bevy::prelude::*;
use bevy::tasks::prelude::*;
use bevy::time::FixedTimestep;
use bevy::math::Vec3Swizzles;

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
    .with_system(select_on_click.after(System::Integrate))
}

struct Buckets<T, I> where I: Iterator<Item = T> {
  batch_size: usize,
  coords: I,
}

impl<T, I> Buckets<T, I> where I: Iterator<Item = T> {
  fn new(batch_size: usize, coords: I) -> Self {
    Buckets { batch_size, coords }
  }
}

impl<T, I> ParallelIterator<std::vec::IntoIter<T>> for Buckets<T, I> where
  I: Iterator<Item = T> + Send,
  T: Send
{
  fn next_batch(&mut self) -> Option<std::vec::IntoIter<T>> {
    let result = (0..self.batch_size)
      .filter_map(|_| self.coords.next())
      .collect::<Vec<_>>();
    if result.is_empty() {
      None
    } else {
      Some(result.into_iter())
    }
  }
}

pub fn compute_forces(
  args: Res<ProgramArgs>,
  particle_spec: Res<ParticleSpec>,
  sim_region: Res<SimRegion>,
  pool: Res<ComputeTaskPool>,
  state: Res<State<SimState>>,
  mut particles_out: Query<(Entity, &Transform, &mut Acceleration, &InteractionId)>,
  particles_in: Query<(Entity, &Transform, &InteractionId)>
) {
  if state.current() == &SimState::Paused {
    return;
  }
  let coords_iter = sim_region.index.iter()
    .filter(|(_, v)| !v.is_empty())
    .map(|(&k, v)| (v, sim_region.get_entities(k).clone()));
  let update_batches = Buckets::new(args.parallel_batch_size, coords_iter)
    .map(|(out_bucket, in_buckets)| {
      let mut result = Vec::with_capacity(out_bucket.len());
      for (entity, transform, _, interaction) in out_bucket.iter().map(|&entity| particles_out.get(entity).unwrap()) {
        let mut acceleration = Vec2::ZERO;
        for (other_entity, other_transform, other_interaction) in in_buckets.clone().map(|other_entity| particles_in.get(other_entity).unwrap()) {
          if entity == other_entity {
            continue
          }
          let delta = sim_region.get_corrected_position_delta(transform.translation.xy(), other_transform.translation.xy());
          let distance_sq: f32 = delta.length_squared();
          if distance_sq > 1600.0 {
            continue;
          }
          let distance = distance_sq.sqrt();
          let distance_unit_vector = delta / distance;
          if distance < 10.0 {
            let safety_margin_repulsion_force = (1000.0 - 100.0 * distance) * distance_unit_vector;
            acceleration -= safety_margin_repulsion_force;
          } else {
            acceleration += triangular_kernel(particle_spec.interactions[other_interaction.0].force_coeffs[interaction.0], 30.0, 10.0, distance) * distance_unit_vector;
          }
        }
        result.push((entity, acceleration));
      }
      result
    }).collect::<Vec<_>>(&pool);

  for batch in update_batches {
    for (entity, accel) in batch {
      particles_out.get_mut(entity).unwrap().2.0 += accel;
    }
  }
}

pub fn compute_friction(state: Res<State<SimState>>, mut particles: Query<(&Transform, &mut LastPosition, &mut Acceleration)>) {
  if state.current() == &SimState::Paused {
    return;
  }
  for (transform, mut last_pos, mut acceleration) in particles.iter_mut() {
    let velocity = (transform.translation.xy() - last_pos.0) / DELTA_TIME as f32;
    if velocity.length_squared() < VELOCITY_THRESHOLD {
      last_pos.0 = transform.translation.xy();
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

pub fn integrate(
  state: Res<State<SimState>>, mut query: Query<(&mut Acceleration, &mut Transform, &mut LastPosition)>) {
  if state.current() == &SimState::Paused {
    return;
  }
  let dt_sq = (DELTA_TIME * DELTA_TIME) as f32;
  for (mut acceleration, mut transform, mut last_pos) in query.iter_mut() {
    let new_pos =
        2.0 * transform.translation.xy() - last_pos.0 + acceleration.0 * dt_sq;
    acceleration.0 = Vec2::ZERO;
    last_pos.0 = transform.translation.xy();
    transform.translation = new_pos.extend(0.0);
  }
}

pub fn wrap_around(
  state: Res<State<SimState>>,
  mut sim_region: ResMut<SimRegion>,
  mut query: Query<(Entity, &mut Transform, &mut LastPosition)>
) {
  if state.current() == &SimState::Paused {
    return;
  }
  for (entity, mut transform, mut last_pos) in query.iter_mut() {
    let x_old = last_pos.0.x;
    let y_old = last_pos.0.y;
    let adjustment = sim_region.get_wrap_around_adjustment(transform.translation.xy());
    transform.translation += adjustment.extend(0.0);
    last_pos.0 += adjustment;
    let x_new = transform.translation.x;
    let y_new = transform.translation.y;
    sim_region.move_entity(entity, x_old, y_old, x_new, y_new);
  }
}

pub fn update_shape(state: Res<State<SimState>>, mut query: Query<(&mut Transform, &LastPosition)>) {
  if state.current() == &SimState::Paused {
    return;
  }
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
  let angle = velocity.angle_between(Vec2::new(1.0, 0.0));
  Quat::from_rotation_z(-angle)
}

#[derive(Default, Debug)]
struct SelectedGizmo {
  id: Option<Entity>,
  translation: Vec3,
}

fn select_on_click(
  mouse_buttons: Res<Input<MouseButton>>,
  windows: Res<Windows>,
  camera_query: Query<&Transform, With<MainCamera>>,
  particles: Query<(Entity, &Transform, &Children), With<Acceleration>>,
  sim_region: Res<SimRegion>,
  mut gizmos: Query<(Option<&Selection>, Option<&Highlight>, &mut Visibility), Or<(With<Selection>, With<Highlight>)>>,
  mut selected_gizmo: Local<SelectedGizmo>,
) {
  let window = windows.get_primary().unwrap();
  let cursor_position_opt = window.cursor_position();
  if cursor_position_opt.is_none() {
    return;
  }
  let cursor_position = cursor_position_opt.unwrap();
  let size = Vec2::new(window.width() as f32, window.height() as f32);
  let cursor_position_offset = cursor_position - size / 2.0;
  let camera_transform = camera_query.single();
  let world_position = camera_transform.compute_matrix() * cursor_position_offset.extend(0.0).extend(1.0);

  if mouse_buttons.just_pressed(MouseButton::Left) {
    for (_, _, mut visible) in gizmos.iter_mut().filter(|(s, _, _)| s.is_some()) {
      visible.is_visible = false;
    }
    selected_gizmo.id = None;
    for (particle, transform, children) in particles.iter() {
      if (transform.translation - world_position.truncate()).truncate().length_squared() > 16.0 {
        continue;
      }
      selected_gizmo.id = Some(particle);
      selected_gizmo.translation = transform.translation;
      for &child in children.iter() {
        if let Ok((Some(_), None, mut visible)) = gizmos.get_mut(child) {
          visible.is_visible = true;
        }
      }
      break;
    }
  }

  if let Some(particle) = selected_gizmo.id {
    let (_, transform, _) = particles.get(particle).unwrap();
    selected_gizmo.translation = transform.translation;
    for (_, _, mut visible) in gizmos.iter_mut().filter(|(_, h, _)| h.is_some()) {
      visible.is_visible = false;
    }
    let neighbour_ids = sim_region.get_entities_by_position(selected_gizmo.translation.x, selected_gizmo.translation.y);
    for nid in neighbour_ids {
      let (_, _, children) = particles.get(nid).unwrap();
      for &child in children.iter() {
        if let Ok((None, Some(_), mut visible)) = gizmos.get_mut(child) {
          visible.is_visible = true;
        }
      }
    }
  }
}
