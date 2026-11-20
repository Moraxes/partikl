use super::*;
use crate::config::INTERACTION_RADIUS;
use rand::prelude::*;
use std::collections::HashSet;

fn entities(n: usize) -> Vec<Entity> {
  let mut world = World::new();
  (0..n).map(|_| world.spawn_empty().id()).collect()
}

fn neighbours(grid: &SpatialGrid, x: f32, y: f32) -> Vec<Entity> {
  grid.get_entities_by_position(x, y).collect()
}

// --- zawijanie pozycji ---

#[test]
fn wrap_adjustment_is_zero_inside_world() {
  let grid = SpatialGrid::new(200.0, 100.0, 10.0);
  assert_eq!(grid.get_wrap_around_adjustment(Vec2::new(0.0, 0.0)), Vec2::ZERO);
  assert_eq!(grid.get_wrap_around_adjustment(Vec2::new(100.0, 50.0)), Vec2::ZERO);
  assert_eq!(grid.get_wrap_around_adjustment(Vec2::new(-100.0, -50.0)), Vec2::ZERO);
}

#[test]
fn wrap_adjustment_moves_by_world_size_per_axis() {
  let grid = SpatialGrid::new(200.0, 100.0, 10.0);
  assert_eq!(grid.get_wrap_around_adjustment(Vec2::new(101.0, 0.0)), Vec2::new(-200.0, 0.0));
  assert_eq!(grid.get_wrap_around_adjustment(Vec2::new(-101.0, 0.0)), Vec2::new(200.0, 0.0));
  assert_eq!(grid.get_wrap_around_adjustment(Vec2::new(0.0, 51.0)), Vec2::new(0.0, -100.0));
  assert_eq!(grid.get_wrap_around_adjustment(Vec2::new(0.0, -51.0)), Vec2::new(0.0, 100.0));
  assert_eq!(
    grid.get_wrap_around_adjustment(Vec2::new(101.0, -51.0)),
    Vec2::new(-200.0, 100.0)
  );
}

#[test]
fn corrected_delta_takes_the_short_way_around() {
  let grid = SpatialGrid::new(200.0, 100.0, 10.0);
  // 95 -> -95 w osi x to 10 jednostek przez krawędź, nie 190.
  let delta = grid.get_corrected_position_delta(Vec2::new(95.0, 0.0), Vec2::new(-95.0, 0.0));
  assert_eq!(delta, Vec2::new(10.0, 0.0));
  let delta = grid.get_corrected_position_delta(Vec2::new(-95.0, 0.0), Vec2::new(95.0, 0.0));
  assert_eq!(delta, Vec2::new(-10.0, 0.0));
  let delta = grid.get_corrected_position_delta(Vec2::new(0.0, 45.0), Vec2::new(0.0, -45.0));
  assert_eq!(delta, Vec2::new(0.0, 10.0));
}

#[test]
fn corrected_delta_is_plain_difference_for_close_points() {
  let grid = SpatialGrid::new(200.0, 100.0, 10.0);
  let delta = grid.get_corrected_position_delta(Vec2::new(10.0, 10.0), Vec2::new(-20.0, 15.0));
  assert_eq!(delta, Vec2::new(-30.0, 5.0));
}

// --- rebuild ---

/// Siatka z cząstkami `(encja, x, y, typ)`.
fn grid_with(width: f32, height: f32, cell: f32, particles: &[(Entity, f32, f32, usize)]) -> SpatialGrid {
  let mut grid = SpatialGrid::new(width, height, cell);
  grid.rebuild(particles.iter().map(|&(e, x, y, t)| (e, Vec2::new(x, y), InteractionId(t))));
  grid
}

#[test]
fn rebuilt_entity_is_found_near_its_position() {
  let e = entities(1);
  let grid = grid_with(200.0, 200.0, 20.0, &[(e[0], 5.0, 5.0, 0)]);
  assert_eq!(neighbours(&grid, 5.0, 5.0), vec![e[0]]);
  assert_eq!(neighbours(&grid, 25.0, 5.0), vec![e[0]]);
  assert!(neighbours(&grid, 60.0, 5.0).is_empty());
}

#[test]
fn rebuild_replaces_previous_content() {
  let e = entities(2);
  let mut grid = grid_with(400.0, 400.0, 20.0, &[(e[0], 0.0, 0.0, 0), (e[1], 1.0, 1.0, 0)]);
  grid.rebuild([(e[0], Vec2::new(100.0, 0.0), InteractionId(0))]);
  assert!(neighbours(&grid, 0.0, 0.0).is_empty());
  assert_eq!(neighbours(&grid, 100.0, 0.0), vec![e[0]]);
  assert_eq!(grid.len(), 1);
}

#[test]
#[should_panic]
fn rebuild_with_position_outside_world_panics() {
  let e = entities(1);
  grid_with(200.0, 200.0, 20.0, &[(e[0], 500.0, 0.0, 0)]);
}

/// Jedyny sąsiad w okolicy `(x, y)`: (pozycja, typ).
fn stored(grid: &SpatialGrid, x: f32, y: f32) -> (Vec2, usize) {
  let found: Vec<_> = grid.get_neighbours_by_position(x, y).collect();
  assert_eq!(found.len(), 1);
  (found[0].position, found[0].interaction.0)
}

#[test]
fn rebuilt_entity_keeps_position_and_type() {
  let e = entities(1);
  let grid = grid_with(200.0, 200.0, 20.0, &[(e[0], 5.0, -3.0, 2)]);
  assert_eq!(stored(&grid, 5.0, -3.0), (Vec2::new(5.0, -3.0), 2));
}

#[test]
fn arrays_are_sorted_by_cell_and_padded_with_nan() {
  let size = Vec2::new(500.0, 330.0);
  let mut rng = StdRng::seed_from_u64(6);
  let points = random_points(&mut rng, 300, size);
  let ids = entities(points.len());
  let grid = build_grid(&ids, &points, size);
  assert_eq!(grid.len(), points.len());
  let mut seen = 0;
  for c in 0..grid.cell_count() {
    for i in grid.cell_range(c) {
      assert_eq!(grid.cell_at(grid.xs()[i], grid.ys()[i]), c);
      seen += 1;
    }
  }
  assert_eq!(seen, points.len());
  assert_eq!(grid.xs().len(), points.len() + SpatialGrid::PADDING);
  assert!(grid.xs()[points.len()..].iter().chain(&grid.ys()[points.len()..]).all(|x| x.is_nan()));
}

#[test]
fn rebuild_keeps_given_order_within_a_cell() {
  let e = entities(3);
  let grid = grid_with(200.0, 200.0, 20.0, &[(e[2], 1.0, 1.0, 0), (e[0], 2.0, 2.0, 0), (e[1], 3.0, 3.0, 0)]);
  let c = grid.cell_at(1.0, 1.0);
  assert_eq!(&grid.entities()[grid.cell_range(c)], &[e[2], e[0], e[1]]);
}

// --- sąsiedztwo przez krawędzie świata ---

#[test]
fn neighbours_wrap_across_edges() {
  let e = entities(2);
  let grid = grid_with(400.0, 400.0, 20.0, &[(e[0], 195.0, 0.0, 0), (e[1], -195.0, 0.0, 0)]);
  assert!(neighbours(&grid, 195.0, 0.0).contains(&e[1]));
  assert!(neighbours(&grid, -195.0, 0.0).contains(&e[0]));
}

#[test]
fn neighbours_wrap_across_corners() {
  let e = entities(2);
  let grid = grid_with(400.0, 400.0, 20.0, &[(e[0], 195.0, 195.0, 0), (e[1], -195.0, -195.0, 0)]);
  assert!(neighbours(&grid, 195.0, 195.0).contains(&e[1]));
  assert!(neighbours(&grid, -195.0, -195.0).contains(&e[0]));
}

// --- własność: wynik zgodny z brute-force na torusie ---

/// Odległość na torusie, liczona niezależnie od `SpatialGrid`.
fn torus_distance(a: Vec2, b: Vec2, size: Vec2) -> f32 {
  let wrap = |d: f32, s: f32| {
    let d = d.abs();
    d.min(s - d)
  };
  Vec2::new(wrap(a.x - b.x, size.x), wrap(a.y - b.y, size.y)).length()
}

fn random_points(rng: &mut StdRng, n: usize, size: Vec2) -> Vec<Vec2> {
  (0..n)
    .map(|_| {
      Vec2::new(
        rng.random_range(-size.x / 2.0..size.x / 2.0),
        rng.random_range(-size.y / 2.0..size.y / 2.0),
      )
    })
    .collect()
}

/// Każda para w zasięgu `radius` musi być zwrócona, bez duplikatów.
fn assert_neighbours_match_brute_force(
  grid: &SpatialGrid,
  ids: &[Entity],
  points: &[Vec2],
  size: Vec2,
  radius: f32,
) {
  for (i, p) in points.iter().enumerate() {
    let found: Vec<Entity> = grid.get_entities_by_position(p.x, p.y).collect();
    let unique: HashSet<Entity> = found.iter().cloned().collect();
    assert_eq!(found.len(), unique.len(), "duplikaty w sąsiedztwie {i} przy {p}");
    for (j, q) in points.iter().enumerate() {
      if torus_distance(*p, *q, size) <= radius {
        assert!(
          unique.contains(&ids[j]),
          "brak sąsiada {j} ({q}) dla {i} ({p}), odległość {}",
          torus_distance(*p, *q, size)
        );
      }
    }
  }
}

fn build_grid(ids: &[Entity], points: &[Vec2], size: Vec2) -> SpatialGrid {
  let mut grid = SpatialGrid::new(size.x, size.y, INTERACTION_RADIUS);
  grid.rebuild(ids.iter().zip(points).map(|(id, p)| (*id, *p, InteractionId(0))));
  grid
}

#[test]
fn random_particles_match_brute_force() {
  let size = Vec2::new(2560.0, 1440.0);
  let mut rng = StdRng::seed_from_u64(1);
  let points = random_points(&mut rng, 1500, size);
  let ids = entities(points.len());
  let grid = build_grid(&ids, &points, size);
  assert_neighbours_match_brute_force(&grid, &ids, &points, size, INTERACTION_RADIUS);
}

#[test]
fn particles_near_edges_match_brute_force() {
  let size = Vec2::new(2560.0, 1440.0);
  let mut rng = StdRng::seed_from_u64(2);
  // Gęsto przy krawędziach i rogach, gdzie zawijanie jest najbardziej podatne na błędy.
  let points: Vec<Vec2> = random_points(&mut rng, 1500, size)
    .into_iter()
    .map(|p| {
      // Przyciąga współrzędną do pasa przy krawędzi (3 komórki szerokości).
      let snap = |v: f32, half: f32| {
        let depth = ((half - v.abs()) % (3.0 * INTERACTION_RADIUS)).max(0.01);
        (half - depth).copysign(v)
      };
      Vec2::new(snap(p.x, size.x / 2.0), snap(p.y, size.y / 2.0))
    })
    .collect();
  let ids = entities(points.len());
  let grid = build_grid(&ids, &points, size);
  assert_neighbours_match_brute_force(&grid, &ids, &points, size, INTERACTION_RADIUS);
}

#[test]
fn world_not_divisible_by_cell_size_matches_brute_force() {
  let size = Vec2::new(500.0, 330.0);
  let mut rng = StdRng::seed_from_u64(3);
  let points = random_points(&mut rng, 400, size);
  let ids = entities(points.len());
  let grid = build_grid(&ids, &points, size);
  assert_neighbours_match_brute_force(&grid, &ids, &points, size, INTERACTION_RADIUS);
}

#[test]
fn match_brute_force_after_moves_across_edges() {
  let size = Vec2::new(2560.0, 1440.0);
  let mut rng = StdRng::seed_from_u64(4);
  let mut points = random_points(&mut rng, 800, size);
  let ids = entities(points.len());
  let mut grid = build_grid(&ids, &points, size);
  for _ in 0..50 {
    for p in points.iter_mut() {
      let mut new = *p + Vec2::new(rng.random_range(-60.0..60.0), rng.random_range(-60.0..60.0));
      new += grid.get_wrap_around_adjustment(new);
      *p = new;
    }
    grid.rebuild(ids.iter().zip(&points).map(|(id, p)| (*id, *p, InteractionId(0))));
  }
  assert_neighbours_match_brute_force(&grid, &ids, &points, size, INTERACTION_RADIUS);
  // Zapisane pozycje nadążają za ruchem, także bez zmiany komórki.
  let current: std::collections::HashMap<Entity, Vec2> = ids.iter().copied().zip(points.iter().copied()).collect();
  for p in &points {
    for n in grid.get_neighbours_by_position(p.x, p.y) {
      assert_eq!(n.position, current[&n.entity]);
    }
  }
}

#[test]
fn precomputed_neighbourhood_matches_computed_in_the_same_order() {
  for (width, height) in [(400.0, 400.0), (500.0, 330.0), (2560.0, 1440.0)] {
    let grid = SpatialGrid::new(width, height, INTERACTION_RADIUS);
    for iy in grid.bottom..=grid.top {
      for ix in grid.left..=grid.right {
        let c = grid.cell_index((ix, iy)).unwrap();
        let range = grid.neighbour_starts[c] as usize..grid.neighbour_starts[c + 1] as usize;
        let precomputed: Vec<usize> = grid.neighbour_cells[range].iter().map(|&i| i as usize).collect();
        let computed: Vec<usize> = grid.neighbour_cells_of((ix, iy)).collect();
        assert_eq!(precomputed, computed, "komórka ({ix}, {iy}) w świecie {width}x{height}");
      }
    }
  }
}

#[test]
fn neighbour_runs_cover_the_same_cells_once() {
  for (width, height) in [(400.0, 400.0), (500.0, 330.0), (2560.0, 1440.0)] {
    let grid = SpatialGrid::new(width, height, INTERACTION_RADIUS);
    for c in 0..grid.cell_count() {
      let mut expected: Vec<u32> = grid.neighbour_cells_at(c).to_vec();
      expected.sort_unstable();
      let from_runs: Vec<u32> = grid.neighbour_runs(c).iter().flat_map(|r| r.first..r.last).collect();
      assert_eq!(from_runs, expected, "komórka {c} w świecie {width}x{height}");
    }
  }
}

#[test]
fn run_shift_gives_the_same_delta_as_wrapping() {
  for (width, height) in [(400.0, 400.0), (500.0, 330.0), (2560.0, 1440.0)] {
    let size = Vec2::new(width, height);
    let mut rng = StdRng::seed_from_u64(5);
    let points = random_points(&mut rng, 600, size);
    let ids = entities(points.len());
    let grid = build_grid(&ids, &points, size);
    let mut shifted = 0;
    for p in &points {
      for run in grid.neighbour_runs(grid.cell_at(p.x, p.y)) {
        let cells = grid.cell_starts()[run.first as usize] as usize..grid.cell_starts()[run.last as usize] as usize;
        for i in cells {
          let q = Vec2::new(grid.xs()[i], grid.ys()[i]);
          let delta = (q - *p) + run.shift;
          assert_eq!(delta, grid.get_corrected_position_delta(*p, q), "{p} → {q}");
          shifted += (run.shift != Vec2::ZERO) as usize;
        }
      }
    }
    assert!(shifted > 0, "żadna para przez szew w świecie {width}x{height}");
  }
}

#[test]
fn every_reach_matches_brute_force_including_edges() {
  for reach in 1..=3 {
    for (size, seed) in [(Vec2::new(2560.0, 1440.0), 7), (Vec2::new(500.0, 330.0), 8), (Vec2::new(410.0, 290.0), 9)] {
      let mut rng = StdRng::seed_from_u64(seed);
      let mut points = random_points(&mut rng, 600, size);
      // Połowa przy krawędziach i rogach.
      for p in points.iter_mut().step_by(2) {
        let mut snap = |v: f32, half: f32| (half - rng.random_range(0.01..2.0 * INTERACTION_RADIUS)).copysign(v);
        *p = Vec2::new(snap(p.x, size.x / 2.0), snap(p.y, size.y / 2.0));
      }
      let ids = entities(points.len());
      let mut grid = SpatialGrid::with_reach(size.x, size.y, INTERACTION_RADIUS, reach);
      grid.rebuild(ids.iter().zip(&points).map(|(id, p)| (*id, *p, InteractionId(0))));
      assert_neighbours_match_brute_force(&grid, &ids, &points, size, INTERACTION_RADIUS);
      for p in &points {
        for run in grid.neighbour_runs(grid.cell_at(p.x, p.y)) {
          for i in grid.cell_starts()[run.first as usize] as usize..grid.cell_starts()[run.last as usize] as usize {
            let q = Vec2::new(grid.xs()[i], grid.ys()[i]);
            assert_eq!((q - *p) + run.shift, grid.get_corrected_position_delta(*p, q), "reach {reach}, {p} → {q}");
          }
        }
      }
    }
  }
}
