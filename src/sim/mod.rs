use bevy::app::{TaskPoolOptions, TaskPoolThreadAssignmentPolicy};
use bevy::math::Vec3Swizzles;
use bevy::prelude::*;
use bevy::tasks::available_parallelism;

mod components;
mod forces;
mod grid;
mod options;

use bevy::ecs::system::SystemId;

pub use components::*;
pub use forces::{ForceData, compute_forces};
pub use grid::{Neighbour, SpatialGrid};
pub use options::*;

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
  Grid,
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

pub struct SimPlugin;

impl Plugin for SimPlugin {
  fn build(&self, app: &mut App) {
    app
      .insert_resource(Time::<Fixed>::from_seconds(DELTA_TIME))
      .init_state::<SimState>()
      .init_resource::<SimOptions>()
      .init_resource::<ForceData>()
      .configure_sets(
        FixedUpdate,
        (
          SimSet::Forces,
          SimSet::Friction,
          SimSet::Integrate,
          SimSet::Wrap,
          SimSet::Grid,
          SimSet::Shape,
        )
          .chain()
          .run_if(in_state(SimState::Running)),
      )
      .add_systems(
        FixedUpdate,
        (
          forces_variant::<false, SPRING>(),
          forces_variant::<false, RESOLVE>(),
          forces_variant::<false, ADAPTIVE_RANGE>(),
          forces_variant::<true, SPRING>(),
          forces_variant::<true, RESOLVE>(),
          forces_variant::<true, ADAPTIVE_RANGE>(),
          compute_friction.in_set(SimSet::Friction),
          integrate.in_set(SimSet::Integrate),
          wrap_around.in_set(SimSet::Wrap),
          rebuild_grid.in_set(SimSet::Grid),
          update_shape.in_set(SimSet::Shape),
        ),
      );
  }
}

/// Tarcie kwadratowe (`v' = −c|v|v`) na prędkości, którą cząstka miałaby po tym kroku, a nie
/// na różnicy pozycji z poprzedniego kroku. Wynik jako przyspieszenie zastępcze, więc
/// `integrate` przesunie cząstkę o przemieszczenie po kroku już stłumione (szczegóły w
/// `friction_acceleration`). Prędkość poniżej `VELOCITY_THRESHOLD` jest zerowana.
/// Wariant `compute_forces` działający tylko przy pasujących `SimOptions`.
fn forces_variant<const AVERAGE: bool, const OVERLAP: u8>() -> bevy::ecs::schedule::ScheduleConfigs<bevy::ecs::system::ScheduleSystem> {
  compute_forces::<AVERAGE, OVERLAP>
    .run_if(|options: Res<SimOptions>| options.selects(AVERAGE, OVERLAP))
    .in_set(SimSet::Forces)
}

/// Rejestruje wariant `compute_forces` dla `options` (do ręcznego uruchamiania kroku: bench,
/// testy). `ForceData` musi być w świecie.
pub fn register_forces(world: &mut World, options: SimOptions) -> SystemId {
  match (options.average_forces, options.overlap) {
    (false, Overlap::Spring) => world.register_system(compute_forces::<false, SPRING>),
    (false, Overlap::Resolve) => world.register_system(compute_forces::<false, RESOLVE>),
    (false, Overlap::AdaptiveRange) => world.register_system(compute_forces::<false, ADAPTIVE_RANGE>),
    (true, Overlap::Spring) => world.register_system(compute_forces::<true, SPRING>),
    (true, Overlap::Resolve) => world.register_system(compute_forces::<true, RESOLVE>),
    (true, Overlap::AdaptiveRange) => world.register_system(compute_forces::<true, ADAPTIVE_RANGE>),
  }
}

pub fn compute_friction(
  mut particles: Query<(&Transform, &mut LastPosition, &mut Acceleration)>,
) {
  let dt = DELTA_TIME as f32;
  for (transform, mut last_pos, mut acceleration) in particles.iter_mut() {
    let position = transform.translation.xy();
    let velocity = (position - last_pos.0) / dt;
    if velocity.length_squared() < VELOCITY_THRESHOLD {
      last_pos.0 = position;
    }
    acceleration.0 = friction_acceleration(position - last_pos.0, acceleration.0, dt * dt);
  }
}

/// Przyspieszenie zastępcze z tarciem dla cząstki o przemieszczeniu w poprzednim kroku
/// `displacement` i przyspieszeniu od sił `acceleration`.
///
/// Bez tarcia Verlet przesunąłby cząstkę o `d̃ = displacement + acceleration·dt²`. Samo tarcie
/// kwadratowe ma dokładne rozwiązanie `|v(t)| = |v₀| / (1 + c|v₀|t)`, więc po kroku
/// przemieszczenie to `d̃ / (1 + c|d̃|)` (`|v₀|·dt = |d̃|`): nigdy nie zmienia kierunku, a pod
/// stałą siłą prędkość dąży do `√(a/c)` jak w modelu ciągłym. Jawne `a −= c|v|v` z prędkości
/// poprzedniego kroku mnożyło przemieszczenie przez `1 − c|d|`, co przy dużej prędkości
/// zmienia znak przemieszczenia i wzbudza drgania, a siły z bieżącego kroku zostawiało
/// nietłumione do następnego.
fn friction_acceleration(displacement: Vec2, acceleration: Vec2, dt_sq: f32) -> Vec2 {
  let free = displacement + acceleration * dt_sq;
  let damped = free / (1.0 + FRICTION_COEFF * free.length());
  (damped - displacement) / dt_sq
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
  grid: Res<SpatialGrid>,
  mut query: Query<(&mut Transform, &mut LastPosition)>,
) {
  for (mut transform, mut last_pos) in query.iter_mut() {
    let adjustment = grid.get_wrap_around_adjustment(transform.translation.xy());
    if adjustment != Vec2::ZERO {
      transform.translation += adjustment.extend(0.0);
      last_pos.0 += adjustment;
    }
  }
}

/// Siatka z pozycji po kroku (po zawinięciu), dla sił w następnym kroku.
pub fn rebuild_grid(
  mut grid: ResMut<SpatialGrid>,
  query: Query<(Entity, &Transform, &InteractionId)>,
) {
  grid.rebuild(query.iter().map(|(entity, transform, interaction)| {
    (entity, transform.translation.xy(), *interaction)
  }));
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

/// Obrót wokół osi z o kąt θ wektora prędkości `v`, bez funkcji trygonometrycznych (`sincos`
/// był ok. 2/3 kosztu `update_shape`). Kwaternion to `(0, 0, sin θ/2, cos θ/2)`, a ta para
/// jest równoległa do `(v.y, |v| + v.x)` (bo `v.y = 2|v| sin θ/2 cos θ/2`,
/// `|v| + v.x = 2|v| cos² θ/2`) oraz, z dokładnością do znaku `v.y`, do `(|v| − v.x, v.y)`
/// (`|v| − v.x = 2|v| sin² θ/2`). Pierwsza postać dla `v.x ≥ 0`, druga dla `v.x < 0`, żeby
/// nie odejmować bliskich liczb; w obu pierwsza składowa ≥ |v| > 0, więc normalizacja jest
/// bezpieczna. `v` przeciwne do osi x daje obrót o ±π.
fn rotation_from_velocity(velocity: Vec2) -> Quat {
  let length = velocity.length();
  let half = if velocity.x >= 0.0 {
    Vec2::new(velocity.y, length + velocity.x)
  } else {
    velocity.y.signum() * Vec2::new(length - velocity.x, velocity.y)
  };
  let half = half / half.length();
  Quat::from_xyzw(0.0, 0.0, half.x, half.y)
}

#[cfg(test)]
mod tests;
