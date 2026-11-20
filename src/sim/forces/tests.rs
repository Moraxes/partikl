use bevy::prelude::*;
use rand::prelude::*;

use super::{ForceData, compute_forces};
use crate::config::*;
use crate::sim::{Acceleration, Crowding, InteractionId, Overlap, SPRING, SimOptions, SpatialGrid, register_forces};
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
  world.init_resource::<ForceData>();
  world.insert_resource(ParticleSpec {
    interactions: spec.interactions.iter().map(|i| Interaction { force_coeffs: i.force_coeffs.clone() }).collect(),
  });
  for _ in 0..times {
    world.run_system_cached(compute_forces::<false, SPRING>).unwrap();
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
fn dense_cluster_matches_reference_on_average_over_sampling_offsets() {
  let mut rng = StdRng::seed_from_u64(13);
  let spec = spec(&mut rng, 3);
  // Gęste skupisko przy rogu świata (pary przez szew, ponad `NEIGHBOUR_CAP` kandydatów, praca
  // powyżej progu par liczonych raz w każdym kroku) i trochę cząstek dookoła.
  let mut points: Vec<(Vec2, usize)> = (0..2000)
    .map(|_| (Vec2::new(SIZE.x / 2.0 - rng.random_range(0.0..30.0), SIZE.y / 2.0 - rng.random_range(0.0..30.0)), rng.random_range(0..3)))
    .collect();
  points.extend(random_points(&mut rng, 200, 3));
  let order: Vec<usize> = (0..points.len()).collect();
  // Co `n`-ty wektor (n ≤ 8) z przesunięciem co krok: po NWW(1..8) krokach każdy wektor
  // policzony tyle samo razy z wagą `n`, więc średnia to pełna suma.
  let steps = 840;
  let total = run_forces_times(&spec, &points, &order, steps);
  let mean: Vec<Vec2> = total.iter().map(|a| *a / steps as f32).collect();
  assert_close(&mean, &spec, &points);
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

#[test]
fn dense_forces_are_bit_identical_between_runs() {
  let mut rng = StdRng::seed_from_u64(15);
  let spec = spec(&mut rng, 3);
  let mut points: Vec<(Vec2, usize)> = (0..3000)
    .map(|_| (Vec2::new(rng.random_range(-60.0..60.0), rng.random_range(-60.0..60.0)), rng.random_range(0..3)))
    .collect();
  points.extend(random_points(&mut rng, 1000, 3));
  let order: Vec<usize> = (0..points.len()).collect();
  let first = run_forces_times(&spec, &points, &order, 3);
  for _ in 0..3 {
    let again = run_forces_times(&spec, &points, &order, 3);
    assert!(first.iter().zip(&again).all(|(a, b)| a.x.to_bits() == b.x.to_bits() && a.y.to_bits() == b.y.to_bits()));
  }
}

/// Przebiegi wariantu `options` (pierwszy w trybie par liczonych raz, kolejne jednostronnie,
/// jak w `run_forces_times`; `one_sided`: od razu jednostronnie) z `Crowding` = `crowding`
/// dla wszystkich cząstek. Wynik: przyspieszenia, pozycje i `Crowding` w kolejności `points`.
struct VariantRun {
  accelerations: Vec<Vec2>,
  positions: Vec<Vec2>,
  crowding: Vec<f32>,
}

fn run_variant(
  spec: &ParticleSpec,
  points: &[(Vec2, usize)],
  options: SimOptions,
  crowding: f32,
  times: usize,
  one_sided: bool,
) -> VariantRun {
  let mut world = World::new();
  bevy::tasks::ComputeTaskPool::get_or_init(bevy::tasks::TaskPool::default);
  let entities: Vec<Entity> = points
    .iter()
    .map(|&(p, t)| {
      world
        .spawn((Acceleration(Vec2::ZERO), Crowding(crowding), InteractionId(t), Transform::from_translation(p.extend(0.0))))
        .id()
    })
    .collect();
  let mut grid = SpatialGrid::new(SIZE.x, SIZE.y, INTERACTION_RADIUS);
  grid.rebuild(entities.iter().zip(points).map(|(e, &(p, t))| (*e, p, InteractionId(t))));
  world.insert_resource(grid);
  world.init_resource::<ForceData>();
  if one_sided {
    world.resource_mut::<ForceData>().last_work = 0;
  }
  world.insert_resource(ParticleSpec {
    interactions: spec.interactions.iter().map(|i| Interaction { force_coeffs: i.force_coeffs.clone() }).collect(),
  });
  let system = register_forces(&mut world, options);
  for _ in 0..times {
    world.run_system(system).unwrap();
  }
  VariantRun {
    accelerations: entities.iter().map(|e| world.get::<Acceleration>(*e).unwrap().0).collect(),
    positions: entities.iter().map(|e| world.get::<Transform>(*e).unwrap().translation.truncate()).collect(),
    crowding: entities.iter().map(|e| world.get::<Crowding>(*e).unwrap().0).collect(),
  }
}

/// Sąsiedzi `me` w zasięgu oddziaływania (bez niej samej), na torusie.
fn neighbours(points: &[(Vec2, usize)], me: usize) -> usize {
  let p = points[me].0;
  let wrap = |d: f32, s: f32| if d > s / 2.0 { d - s } else if d < -s / 2.0 { d + s } else { d };
  points
    .iter()
    .filter(|(q, _)| {
      let d = Vec2::new(wrap(q.x - p.x, SIZE.x), wrap(q.y - p.y, SIZE.y)).length();
      d > 0.0 && d <= INTERACTION_RADIUS
    })
    .count()
}

#[test]
fn averaged_forces_match_reference_scaled_by_neighbour_count_in_both_modes() {
  let mut rng = StdRng::seed_from_u64(16);
  let spec = spec(&mut rng, 3);
  // Skupisko z ponad `AVERAGE_NEIGHBOURS` sąsiadami, ale poniżej limitu próbkowania.
  let mut points: Vec<(Vec2, usize)> = (0..150)
    .map(|_| (Vec2::new(rng.random_range(0.0..40.0), rng.random_range(0.0..40.0)), rng.random_range(0..3)))
    .collect();
  points.extend(random_points(&mut rng, 300, 3));
  assert!((0..points.len()).any(|k| neighbours(&points, k) > AVERAGE_NEIGHBOURS as usize));
  let options = SimOptions { average_forces: true, overlap: Overlap::Spring };
  for one_sided in [false, true] {
    let run = run_variant(&spec, &points, options, 0.0, 1, one_sided);
    for (k, a) in run.accelerations.iter().enumerate() {
      let scale = (AVERAGE_NEIGHBOURS / neighbours(&points, k) as f32).min(1.0);
      let expected = reference(&spec, &points, k) * scale;
      let tolerance = 1e-3 * expected.length().max(1.0);
      assert!(a.distance(expected) <= tolerance, "jednostronnie {one_sided}, cząstka {k}: {a} vs {expected}");
    }
  }
}

#[test]
fn resolve_moves_an_overlapping_pair_apart_without_a_spring_force() {
  let spec = ParticleSpec { interactions: vec![Interaction { force_coeffs: vec![300.0] }] };
  let points = [(Vec2::new(0.0, 0.0), 0), (Vec2::new(4.0, 0.0), 0)];
  let options = SimOptions { average_forces: false, overlap: Overlap::Resolve };
  for one_sided in [false, true] {
    let run = run_variant(&spec, &points, options, 0.0, 1, one_sided);
    // Każda o połowę wnikania (10 − 4) / 2 = 3, przyciąganie zaczyna się dopiero od 20.
    assert!(run.positions[0].distance(Vec2::new(-3.0, 0.0)) < 1e-4, "{:?}", run.positions);
    assert!(run.positions[1].distance(Vec2::new(7.0, 0.0)) < 1e-4, "{:?}", run.positions);
    assert_eq!(run.accelerations, vec![Vec2::ZERO; 2]);
  }
}

#[test]
fn resolve_averages_corrections_over_overlaps() {
  let spec = ParticleSpec { interactions: vec![Interaction { force_coeffs: vec![0.0] }] };
  // Środkowa cząstka nakłada się na obie skrajne symetrycznie: poprawki się znoszą. Skrajne
  // nakładają się tylko na środkową (odstęp 12 między nimi).
  let points = [(Vec2::new(-6.0, 0.0), 0), (Vec2::new(0.0, 0.0), 0), (Vec2::new(6.0, 0.0), 0)];
  let options = SimOptions { average_forces: false, overlap: Overlap::Resolve };
  for one_sided in [false, true] {
    let run = run_variant(&spec, &points, options, 0.0, 1, one_sided);
    let expected = [Vec2::new(-8.0, 0.0), Vec2::ZERO, Vec2::new(8.0, 0.0)];
    for (p, e) in run.positions.iter().zip(expected) {
      assert!(p.distance(e) < 1e-4, "jednostronnie {one_sided}: {:?}", run.positions);
    }
  }
}

#[test]
fn adaptive_range_repels_further_when_crowded_and_smooths_crowding() {
  let spec = ParticleSpec { interactions: vec![Interaction { force_coeffs: vec![0.0] }] };
  let points = [(Vec2::new(0.0, 0.0), 0), (Vec2::new(15.0, 0.0), 0)];
  let options = SimOptions { average_forces: false, overlap: Overlap::AdaptiveRange };
  for one_sided in [false, true] {
    // Mało sąsiadów: zasięg 10, para w odległości 15 się nie odpycha.
    let sparse = run_variant(&spec, &points, options, 0.0, 1, one_sided);
    assert_eq!(sparse.accelerations, vec![Vec2::ZERO; 2]);
    // Tłok (zasięg największy, 20): sprężyna o tej samej sztywności, 100 · (20 − 15).
    let crowded = 4.0 * ADAPTIVE_NEIGHBOURS;
    let run = run_variant(&spec, &points, options, crowded, 1, one_sided);
    let push = REPULSION_STRENGTH * (ADAPTIVE_RANGE_MAX - 15.0);
    assert!(run.accelerations[0].distance(Vec2::new(-push, 0.0)) < 1e-3, "{:?}", run.accelerations);
    assert!(run.accelerations[1].distance(Vec2::new(push, 0.0)) < 1e-3, "{:?}", run.accelerations);
    // Jeden sąsiad: N̄ ← N̄ + α(1 − N̄).
    let expected = crowded + CROWDING_SMOOTHING * (1.0 - crowded);
    assert!(run.crowding.iter().all(|c| (c - expected).abs() < 1e-3), "{:?}", run.crowding);
  }
}

#[test]
fn spring_variant_leaves_crowding_and_positions_untouched() {
  let mut rng = StdRng::seed_from_u64(17);
  let spec = spec(&mut rng, 2);
  let points = random_points(&mut rng, 200, 2);
  let run = run_variant(&spec, &points, SimOptions::default(), 5.0, 1, false);
  assert!(run.crowding.iter().all(|&c| c == 5.0));
  assert!(run.positions.iter().zip(&points).all(|(a, (b, _))| a == b));
  assert_close(&run.accelerations, &spec, &points);
}
