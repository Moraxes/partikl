//! Stany syntetyczne o kontrolowanej gęstości, niezależne od fizyki: ten sam stan przed i po
//! zmianie reguł, więc porównanie mierzy silnik, a nie inną trajektorię.

use std::f32::consts::TAU;
use std::fmt;

use bevy::prelude::*;
use clap::ValueEnum;
use rand::prelude::*;

use partikl::config::GRID_REACH;
use partikl::gpu::{Geometry, GpuParticle, initial_particles};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Scene {
  /// Jednostajnie w świecie, prędkości ±125 (jak start aplikacji), mierzone po `--steps`
  /// krokach rozbiegu zamiast od stałego stanu.
  Evolved,
  /// Jednostajnie w świecie, prędkości ±125; stan stały.
  Uniform,
  /// Plamy zajmujące ok. ¼ świata: gęstość w plamach ×4 względem `uniform`.
  Clusters,
  /// Plamy zajmujące ok. 1/16 świata: gęstość ×16; przy dużym `n` ponad progiem próbkowania
  /// sąsiadów (256 kandydatów), czyli najgorszy przypadek pętli sił.
  Dense,
}

impl fmt::Display for Scene {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(self.to_possible_value().unwrap().get_name())
  }
}

/// Plamy: koła o promieniu `radius` w losowych miejscach, razem zajmujące ułamek `coverage`
/// świata (bez odjęcia nakładania się plam), cząstki rozdzielone po równo i jednostajnie w
/// kole. Gęstość jest więc względna (×1/coverage), a bezwzględną pokazuje `candidates_mean`.
struct Blobs {
  radius: f32,
  coverage: f32,
}

const CLUSTER: Blobs = Blobs {
  radius: 100.0,
  coverage: 1.0 / 4.0,
};
const DENSE: Blobs = Blobs {
  radius: 60.0,
  coverage: 1.0 / 16.0,
};
/// Prędkości w plamach (na oś): małe, jak w osiadłym skupisku.
const BLOB_SPEED: f32 = 10.0;

impl Scene {
  /// Stan sceny (dla `Evolved`: stan startowy przed rozbiegiem).
  pub fn particles(
    self,
    n: usize,
    types: usize,
    world: Vec2,
    rng: &mut impl Rng,
  ) -> Vec<GpuParticle> {
    match self {
      Scene::Evolved | Scene::Uniform => initial_particles(n, types, world, rng),
      Scene::Clusters => blobs(&CLUSTER, n, types, world, rng),
      Scene::Dense => blobs(&DENSE, n, types, world, rng),
    }
  }
}

fn blobs(
  blobs: &Blobs,
  n: usize,
  types: usize,
  world: Vec2,
  rng: &mut impl Rng,
) -> Vec<GpuParticle> {
  let area = std::f32::consts::PI * blobs.radius * blobs.radius;
  let count = ((blobs.coverage * world.x * world.y / area).round() as usize).max(1);
  let centers: Vec<Vec2> = (0..count)
    .map(|_| (Vec2::new(rng.random(), rng.random()) - 0.5) * world)
    .collect();
  (0..n)
    .map(|id| {
      // Punkt jednostajnie w kole: promień ~ √u.
      let (u, angle): (f32, f32) = (rng.random(), rng.random::<f32>() * TAU);
      let offset = blobs.radius * u.sqrt() * Vec2::from_angle(angle);
      let pos = wrap(centers[id % count] + offset, world);
      let velocity = (Vec2::new(rng.random(), rng.random()) * 2.0 - 1.0) * BLOB_SPEED;
      GpuParticle {
        pos: pos.to_array(),
        vel: velocity.to_array(),
        species: rng.random_range(0..types) as u32,
        id: id as u32,
      }
    })
    .collect()
}

/// Pozycja zawinięta do świata `[-w/2, w/2)` (torus).
fn wrap(p: Vec2, world: Vec2) -> Vec2 {
  (p + world / 2.0).rem_euclid(world) - world / 2.0
}

/// Średnia liczba kandydatów na cząstkę (cząstki w sąsiedztwie `(2·GRID_REACH + 1)²` komórek
/// siatki GPU, z nią samą), przed próbkowaniem: miara gęstości sceny.
pub fn candidates_mean(particles: &[GpuParticle], world: Vec2) -> f64 {
  let g = Geometry::new(world);
  let (w, h) = (g.dims.x as i32, g.dims.y as i32);
  let cell = |p: &GpuParticle| {
    let c = ((Vec2::from(p.pos) + world / 2.0) / g.cell_size)
      .floor()
      .as_ivec2();
    c.clamp(IVec2::ZERO, IVec2::new(w - 1, h - 1))
  };
  let mut counts = vec![0u64; (w * h) as usize];
  for p in particles {
    let c = cell(p);
    counts[(c.y * w + c.x) as usize] += 1;
  }
  let reach = GRID_REACH;
  let total: u64 = particles
    .iter()
    .map(|p| {
      let c = cell(p);
      let mut sum = 0;
      for dy in -reach..=reach {
        for dx in -reach..=reach {
          let (x, y) = ((c.x + dx).rem_euclid(w), (c.y + dy).rem_euclid(h));
          sum += counts[(y * w + x) as usize];
        }
      }
      sum
    })
    .sum();
  total as f64 / particles.len().max(1) as f64
}
