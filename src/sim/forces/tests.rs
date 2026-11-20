use bevy::prelude::*;
use rand::prelude::*;

use super::compute_forces;
use crate::config::*;
use crate::sim::{Acceleration, InteractionId, SpatialGrid};
use crate::spec::{Interaction, ParticleSpec};

const SIZE: Vec2 = Vec2::new(500.0, 330.0);

fn spec(rng: &mut StdRng, types: usize) -> ParticleSpec {
  ParticleSpec {
    interactions: (0..types)
      .map(|_| Interaction { force_coeffs: (0..types).map(|_| rng.random_range(-500.0..500.0)).collect() })
      .collect(),
  }
}

/// Siła na `me` liczona wprost w f64, po wszystkich parach, z odległością na torusie.
fn reference(spec: &ParticleSpec, points: &[(Vec2, usize)], me: usize) -> Vec2 {
  let (p, t) = points[me];
  let mut sum = (0.0f64, 0.0f64);
  for &(q, other) in points {
    let wrap = |d: f32, s: f32| if d > s / 2.0 { d - s } else if d < -s / 2.0 { d + s } else { d };
    let (dx, dy) = (wrap(q.x - p.x, SIZE.x) as f64, wrap(q.y - p.y, SIZE.y) as f64);
    let d = (dx * dx + dy * dy).sqrt();
    if d == 0.0 || d > INTERACTION_RADIUS as f64 {
      continue;
    }
    let magnitude = if d < REPULSION_RADIUS as f64 {
      -(REPULSION_STRENGTH as f64) * (REPULSION_RADIUS as f64 - d)
    } else {
      let coeff = spec.interactions[other].force_coeffs[t] as f64;
      coeff * (1.0 - ((d - KERNEL_MIDDLE as f64) / KERNEL_WIDTH as f64).abs()).max(0.0)
    };
    sum.0 += magnitude * dx / d;
    sum.1 += magnitude * dy / d;
  }
  Vec2::new(sum.0 as f32, sum.1 as f32)
}

/// Cząstki w świecie, siatka zbudowana w kolejności `grid_order` (indeksy do `points`),
/// jeden przebieg `compute_forces`; przyspieszenia w kolejności `points`.
fn run_forces(spec: &ParticleSpec, points: &[(Vec2, usize)], grid_order: &[usize]) -> Vec<Vec2> {
  run_forces_times(spec, points, grid_order, 1)
}

/// Jak `run_forces`, `times` przebiegów z rzędu (przyspieszenia się sumują). Pierwszy przebieg
/// liczy pary raz (praca poprzedniego kroku nieznana), kolejne przy małej pracy jednostronnie.
fn run_forces_times(spec: &ParticleSpec, points: &[(Vec2, usize)], grid_order: &[usize], times: usize) -> Vec<Vec2> {
  let mut world = World::new();
  bevy::tasks::ComputeTaskPool::get_or_init(bevy::tasks::TaskPool::default);
  let entities: Vec<Entity> = points
    .iter()
    .map(|&(p, t)| world.spawn((Acceleration(Vec2::ZERO), InteractionId(t), Transform::from_translation(p.extend(0.0)))).id())
    .collect();
  let mut grid = SpatialGrid::new(SIZE.x, SIZE.y, INTERACTION_RADIUS);
  grid.rebuild(grid_order.iter().map(|&k| (entities[k], points[k].0, InteractionId(points[k].1))));
  world.insert_resource(grid);
  world.insert_resource(ParticleSpec {
    interactions: spec.interactions.iter().map(|i| Interaction { force_coeffs: i.force_coeffs.clone() }).collect(),
  });
  for _ in 0..times {
    world.run_system_cached(compute_forces).unwrap();
  }
  entities.iter().map(|e| world.get::<Acceleration>(*e).unwrap().0).collect()
}

fn random_points(rng: &mut StdRng, n: usize, types: usize) -> Vec<(Vec2, usize)> {
  (0..n)
    .map(|_| {
      let p = Vec2::new(rng.random_range(-SIZE.x / 2.0..SIZE.x / 2.0), rng.random_range(-SIZE.y / 2.0..SIZE.y / 2.0));
      (p, rng.random_range(0..types))
    })
    .collect()
}

fn assert_close(actual: &[Vec2], spec: &ParticleSpec, points: &[(Vec2, usize)]) {
  for (k, a) in actual.iter().enumerate() {
    let expected = reference(spec, points, k);
    let tolerance = 1e-3 * expected.length().max(1.0);
    assert!(a.distance(expected) <= tolerance, "cząstka {k}: {a} vs {expected}");
  }
}

#[test]
fn forces_match_pairwise_reference() {
  let mut rng = StdRng::seed_from_u64(11);
  let spec = spec(&mut rng, 3);
  let points = random_points(&mut rng, 600, 3);
  let order: Vec<usize> = (0..points.len()).collect();
  assert_close(&run_forces(&spec, &points, &order), &spec, &points);
}

#[test]
fn forces_reach_the_right_particles_when_grid_order_differs_from_ecs() {
  let mut rng = StdRng::seed_from_u64(12);
  let spec = spec(&mut rng, 3);
  let points = random_points(&mut rng, 400, 3);
  let mut order: Vec<usize> = (0..points.len()).collect();
  order.shuffle(&mut rng);
  assert_close(&run_forces(&spec, &points, &order), &spec, &points);
}

#[test]
fn dense_cluster_matches_reference() {
  let mut rng = StdRng::seed_from_u64(13);
  let spec = spec(&mut rng, 3);
  // Skupisko przy rogu świata (pary przez szew) i trochę cząstek dookoła.
  let mut points: Vec<(Vec2, usize)> = (0..300)
    .map(|_| (Vec2::new(SIZE.x / 2.0 - rng.random_range(0.0..30.0), SIZE.y / 2.0 - rng.random_range(0.0..30.0)), rng.random_range(0..3)))
    .collect();
  points.extend(random_points(&mut rng, 200, 3));
  let order: Vec<usize> = (0..points.len()).collect();
  assert_close(&run_forces(&spec, &points, &order), &spec, &points);
}

#[test]
fn pair_and_one_sided_modes_both_match_reference() {
  let mut rng = StdRng::seed_from_u64(14);
  let spec = spec(&mut rng, 3);
  let points = random_points(&mut rng, 500, 3);
  let order: Vec<usize> = (0..points.len()).collect();
  let twice = run_forces_times(&spec, &points, &order, 2);
  let halves: Vec<Vec2> = twice.iter().map(|a| *a / 2.0).collect();
  assert_close(&halves, &spec, &points);
}
