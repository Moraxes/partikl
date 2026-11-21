//! Testy GPU: jeden krok względem skalarnej wyroczni f64 (wszystkie warianty `SimOptions`)
//! oraz niezmienniki wielu kroków. Działają na prawdziwym adapterze (headless, bez okna);
//! bez GPU test jest pomijany z komunikatem.

use bevy::render::render_resource::ShaderType;

use super::compute::SimUniforms;
use super::headless::Harness;
use super::*;
use crate::sim::{Overlap, SimOptions};
use crate::spec::Interaction;

const WORLD: Vec2 = Vec2::new(500.0, 330.0);

fn spec(rng: &mut StdRng, types: usize) -> ParticleSpec {
  ParticleSpec {
    interactions: (0..types)
      .map(|_| Interaction { force_coeffs: (0..types).map(|_| rng.random_range(-500.0..500.0)).collect() })
      .collect(),
  }
}

/// Jeden krok fizyki wprost w f64, po wszystkich parach, z odległością na torusie.
/// Odpowiada `sim::forces` + `compute_friction` + `integrate` + `wrap_around`, bez próbkowania.
fn reference_step(spec: &ParticleSpec, particles: &[GpuParticle], options: SimOptions, world: Vec2) -> Vec<GpuParticle> {
  let radius = |crowding: f64| {
    (REPULSION_RADIUS as f64 * (crowding / ADAPTIVE_NEIGHBOURS as f64).sqrt()).clamp(REPULSION_RADIUS as f64, ADAPTIVE_RANGE_MAX as f64)
  };
  let wrap = |d: f64, size: f64| if d > size / 2.0 { d - size } else if d < -size / 2.0 { d + size } else { d };
  let dt = DELTA_TIME;
  particles
    .iter()
    .enumerate()
    .map(|(i, me)| {
      let (px, py) = (me.pos[0] as f64, me.pos[1] as f64);
      let (mut fx, mut fy, mut count) = (0.0f64, 0.0f64, 0.0f64);
      let (mut cx, mut cy, mut cn) = (0.0f64, 0.0f64, 0.0f64);
      for (j, other) in particles.iter().enumerate() {
        let dx = wrap(other.pos[0] as f64 - px, world.x as f64);
        let dy = wrap(other.pos[1] as f64 - py, world.y as f64);
        let d = (dx * dx + dy * dy).sqrt();
        if j == i || d == 0.0 || d > INTERACTION_RADIUS as f64 {
          continue;
        }
        let reach = match options.overlap {
          Overlap::AdaptiveRange => radius(me.crowding as f64).max(radius(other.crowding as f64)),
          _ => REPULSION_RADIUS as f64,
        };
        let coeff = spec.interactions[other.species as usize].force_coeffs[me.species as usize] as f64;
        let attraction = coeff * (1.0 - ((d - KERNEL_MIDDLE as f64) / KERNEL_WIDTH as f64).abs()).max(0.0);
        let magnitude = if options.overlap == Overlap::Resolve {
          if d < reach {
            cx += 0.5 * (d - reach) * dx / d;
            cy += 0.5 * (d - reach) * dy / d;
            cn += 1.0;
          }
          attraction
        } else if d < reach {
          -(REPULSION_STRENGTH as f64) * (reach - d)
        } else {
          attraction
        };
        fx += magnitude * dx / d;
        fy += magnitude * dy / d;
        count += 1.0;
      }
      if options.average_forces && count > 0.0 {
        let scale = (AVERAGE_NEIGHBOURS as f64 / count).min(1.0);
        fx *= scale;
        fy *= scale;
      }
      let (mut x, mut y) = (px, py);
      if cn > 0.0 {
        x += cx / cn;
        y += cy / cn;
      }
      let mut crowding = me.crowding as f64;
      if options.overlap == Overlap::AdaptiveRange {
        crowding += CROWDING_SMOOTHING as f64 * (count - crowding);
      }
      let (mut dispx, mut dispy) = (x - me.last[0] as f64, y - me.last[1] as f64);
      if (dispx * dispx + dispy * dispy) / (dt * dt) < VELOCITY_THRESHOLD as f64 {
        dispx = 0.0;
        dispy = 0.0;
      }
      let (freex, freey) = (dispx + fx * dt * dt, dispy + fy * dt * dt);
      let damping = 1.0 + FRICTION_COEFF as f64 * (freex * freex + freey * freey).sqrt();
      let (mut nx, mut ny) = (x + freex / damping, y + freey / damping);
      let (mut lx, mut ly) = (x, y);
      for (n, l, size) in [(&mut nx, &mut lx, world.x as f64), (&mut ny, &mut ly, world.y as f64)] {
        if *n > size / 2.0 {
          *n -= size;
          *l -= size;
        } else if *n < -size / 2.0 {
          *n += size;
          *l += size;
        }
      }
      GpuParticle { pos: [nx as f32, ny as f32], last: [lx as f32, ly as f32], species: me.species, crowding: crowding as f32, id: me.id, pad: 0 }
    })
    .collect()
}

fn random_particles(rng: &mut StdRng, n: usize, types: usize, crowding: bool) -> Vec<GpuParticle> {
  let mut particles = initial_particles(n, types, WORLD, rng);
  if crowding {
    for p in &mut particles {
      p.crowding = rng.random_range(0.0..120.0);
    }
  }
  particles
}

#[test]
fn layout_matches_wgsl() {
  assert_eq!(size_of::<GpuParticle>(), 32);
  assert_eq!(SimUniforms::min_size().get(), 96);
}

#[test]
fn geometry_covers_interaction_radius() {
  let worlds = [
    Vec2::new(500.0, 330.0),
    Vec2::new(2560.0, 1440.0),
    Vec2::new(60.0, 60.0),
    // Więcej niż `MAX_CELLS` komórek o bazowym boku: komórki rosną.
    Vec2::new(7680.0, 4320.0),
    Vec2::new(20_000.0, 20_000.0),
    Vec2::new(500_000.0, 100.0),
  ];
  for world in worlds {
    let g = Geometry::new(world);
    assert!(g.dims.x >= MIN_CELLS && g.dims.y >= MIN_CELLS);
    assert!(g.num_cells() <= MAX_CELLS, "{world}: {}", g.dims);
    if world.x >= 100.0 {
      assert!(g.cell_size.x * GRID_REACH as f32 >= INTERACTION_RADIUS - 1e-3, "{world}");
      assert!(g.cell_size.y * GRID_REACH as f32 >= INTERACTION_RADIUS - 1e-3, "{world}");
    }
  }
}

#[test]
fn one_step_matches_reference() {
  for (average_forces, overlap) in [
    (false, Overlap::Spring),
    (true, Overlap::Spring),
    (false, Overlap::Resolve),
    (true, Overlap::Resolve),
    (false, Overlap::AdaptiveRange),
    (true, Overlap::AdaptiveRange),
  ] {
    let options = SimOptions { average_forces, overlap };
    let mut rng = StdRng::seed_from_u64(7);
    let spec = spec(&mut rng, 4);
    // Mało kandydatów w sąsiedztwie (poniżej progu próbkowania), ale dość gęsto na pary.
    let particles = random_particles(&mut rng, 700, 4, overlap == Overlap::AdaptiveRange);
    let expected = reference_step(&spec, &particles, options, WORLD);
    let Some(mut harness) = Harness::new(options, spec, particles, WORLD) else { return };
    assert_close(&harness.run(1), &expected, &format!("{options:?}"));
  }
}

fn assert_close(actual: &[GpuParticle], expected: &[GpuParticle], context: &str) {
  assert_eq!(actual.len(), expected.len());
  for (i, (a, e)) in actual.iter().zip(expected).enumerate() {
    assert_eq!(a.id, i as u32, "{context}");
    assert_eq!(a.species, e.species);
    for k in 0..2 {
      assert!((a.pos[k] - e.pos[k]).abs() < 1e-3, "{context} cząstka {i}: pos {:?} vs {:?}", a.pos, e.pos);
      assert!((a.last[k] - e.last[k]).abs() < 1e-3, "{context} cząstka {i}: last {:?} vs {:?}", a.last, e.last);
    }
    assert!((a.crowding - e.crowding).abs() < 1e-3, "{context} cząstka {i}: crowding {} vs {}", a.crowding, e.crowding);
  }
}

/// Świat z więcej niż `MAX_CELLS` komórkami o bazowym boku: większe komórki dają ten sam wynik.
#[test]
fn large_world_matches_reference() {
  let world = Vec2::new(7680.0, 4320.0);
  assert!(Geometry::new(world).cell_size.x > INTERACTION_RADIUS / GRID_REACH as f32);
  let options = SimOptions::default();
  let mut rng = StdRng::seed_from_u64(3);
  let spec = spec(&mut rng, 3);
  // Gęsto w pasie, żeby było dość par w zasięgu, i rzadko poza nim.
  let mut particles = initial_particles(3000, 3, world, &mut rng);
  for p in particles.iter_mut().take(2000) {
    p.pos[1] = p.pos[1] * 0.02;
    p.last[1] = p.pos[1];
  }
  let expected = reference_step(&spec, &particles, options, world);
  let Some(mut harness) = Harness::new(options, spec, particles, world) else { return };
  assert_close(&harness.run(1), &expected, "duży świat");
}

#[test]
fn many_steps_keep_invariants() {
  let options = SimOptions { average_forces: true, overlap: Overlap::AdaptiveRange };
  let mut rng = StdRng::seed_from_u64(11);
  let spec = spec(&mut rng, 3);
  // Gęsty stan: ponad 512 kandydatów w sąsiedztwie, więc działa próbkowanie.
  let particles = random_particles(&mut rng, 20_000, 3, false);
  let Some(mut harness) = Harness::new(options, spec, particles, WORLD) else { return };
  let state = harness.run(40);
  let mut seen = vec![false; state.len()];
  for p in &state {
    assert!(p.pos.iter().chain(&p.last).chain(&[p.crowding]).all(|v| v.is_finite()), "NaN/Inf w {p:?}");
    assert!(p.pos[0].abs() <= WORLD.x / 2.0 + 1.0 && p.pos[1].abs() <= WORLD.y / 2.0 + 1.0, "poza światem: {p:?}");
    assert!(!seen[p.id as usize], "powtórzone id {}", p.id);
    seen[p.id as usize] = true;
  }
  assert!(seen.iter().all(|&s| s), "zgubione cząstki");
}

#[test]
fn deterministic_mode_repeats_bit_for_bit() {
  let options = SimOptions { average_forces: false, overlap: Overlap::Spring };
  let run = || {
    let mut rng = StdRng::seed_from_u64(5);
    let spec = spec(&mut rng, 3);
    let particles = random_particles(&mut rng, 5_000, 3, false);
    let mut harness = Harness::new(options, spec, particles, WORLD)?;
    harness.set_deterministic(true);
    Some(harness.run(32))
  };
  let (Some(first), Some(second)) = (run(), run()) else { return };
  assert_eq!(first, second, "trajektorie różnią się mimo trybu deterministycznego");
}
