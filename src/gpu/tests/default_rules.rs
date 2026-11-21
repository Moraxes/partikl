use super::*;

/// Jeden krok fizyki wprost w f64, po wszystkich parach, z odległością na torusie.
/// Ta sama fizyka co `rules/default_rules.wesl`, bez próbkowania sąsiadów; parametry z `spec.params`.
fn reference_step(spec: &ParticleSpec, particles: &[GpuParticle], options: Mode, world: Vec2) -> Vec<GpuParticle> {
  let u = &spec.params;
  let radius = |crowding: f64| {
    (u.repulsion_radius as f64 * (crowding / u.adaptive_neighbours as f64).sqrt()).clamp(u.repulsion_radius as f64, u.adaptive_range_max as f64)
  };
  let wrap = |d: f64, size: f64| if d > size / 2.0 { d - size } else if d < -size / 2.0 { d + size } else { d };
  let dt = u.dt as f64;
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
          _ => u.repulsion_radius as f64,
        };
        let coeff = spec.interactions[other.species as usize].force_coeffs[me.species as usize] as f64;
        let attraction = coeff * (1.0 - ((d - u.kernel_middle as f64) / u.kernel_width as f64).abs()).max(0.0);
        let magnitude = if options.overlap == Overlap::Resolve {
          if d < reach {
            cx += 0.5 * (d - reach) * dx / d;
            cy += 0.5 * (d - reach) * dy / d;
            cn += 1.0;
          }
          attraction
        } else if d < reach {
          -(u.repulsion_strength as f64) * (reach - d)
        } else {
          attraction
        };
        fx += magnitude * dx / d;
        fy += magnitude * dy / d;
        count += 1.0;
      }
      if options.average_forces && count > 0.0 {
        let scale = (u.average_neighbours as f64 / count).min(1.0);
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
        crowding += u.crowding_smoothing as f64 * (count - crowding);
      }
      let (mut dispx, mut dispy) = (x - me.last[0] as f64, y - me.last[1] as f64);
      if (dispx * dispx + dispy * dispy) / (dt * dt) < u.velocity_threshold as f64 {
        dispx = 0.0;
        dispy = 0.0;
      }
      let (freex, freey) = (dispx + fx * dt * dt, dispy + fy * dt * dt);
      let damping = 1.0 + u.friction as f64 * (freex * freex + freey * freey).sqrt();
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
    let options = Mode { average_forces, overlap };
    let mut rng = StdRng::seed_from_u64(7);
    let spec = spec(&mut rng, 4);
    // Mało kandydatów w sąsiedztwie (poniżej progu próbkowania), ale dość gęsto na pary.
    let particles = random_particles(&mut rng, 700, 4, overlap == Overlap::AdaptiveRange);
    let expected = reference_step(&spec, &particles, options, WORLD);
    let Some(mut harness) = Harness::new(options.sim(), spec, particles, WORLD) else { return };
    assert_close(&harness.run(1), &expected, &format!("{options:?}"));
  }
}

/// Świat z więcej niż `MAX_CELLS` komórkami o bazowym boku: większe komórki dają ten sam wynik.
#[test]
fn large_world_matches_reference() {
  let world = Vec2::new(7680.0, 4320.0);
  assert!(Geometry::new(world).cell_size.x > INTERACTION_RADIUS / GRID_REACH as f32);
  let options = Mode::default();
  let mut rng = StdRng::seed_from_u64(3);
  let spec = spec(&mut rng, 3);
  // Gęsto w pasie, żeby było dość par w zasięgu, i rzadko poza nim.
  let mut particles = initial_particles(3000, 3, world, &mut rng);
  for p in particles.iter_mut().take(2000) {
    p.pos[1] *= 0.02;
    p.last[1] = p.pos[1];
  }
  let expected = reference_step(&spec, &particles, options, world);
  let Some(mut harness) = Harness::new(options.sim(), spec, particles, world) else { return };
  assert_close(&harness.run(1), &expected, "duży świat");
}

/// Krok zgodny z wyrocznią dla różnej liczby typów: shader czyta współczynniki sił inaczej dla
/// do 4 typów (`vec4`), do 8 (dwa `vec4`) i więcej (macierz z bufora).
#[test]
fn one_step_matches_reference_for_any_number_of_types() {
  for types in [1, 2, 4, 5, 8, 9, 13] {
    let options = Mode { average_forces: true, overlap: Overlap::Spring };
    let mut rng = StdRng::seed_from_u64(40 + types as u64);
    let spec = spec(&mut rng, types);
    let particles = random_particles(&mut rng, 700, types, false);
    let expected = reference_step(&spec, &particles, options, WORLD);
    let Some(mut harness) = Harness::new(options.sim(), spec, particles, WORLD) else { return };
    assert_close(&harness.run(1), &expected, &format!("{types} typów"));
  }
}

/// Zmiana opcji w trakcie działa od następnego kroku: wariant sił dla nowych opcji.
#[test]
fn options_switch_at_runtime() {
  let first = Mode { average_forces: false, overlap: Overlap::Spring };
  let make_spec = || spec(&mut StdRng::seed_from_u64(21), 4);
  let spec = make_spec();
  let particles = random_particles(&mut StdRng::seed_from_u64(22), 700, 4, true);
  let Some(mut harness) = Harness::new(first.sim(), make_spec(), particles.clone(), WORLD) else { return };
  assert_close(&harness.run(1), &reference_step(&spec, &particles, first, WORLD), "start");
  for next in [
    Mode { average_forces: true, overlap: Overlap::AdaptiveRange },
    Mode { average_forces: false, overlap: Overlap::Resolve },
    first,
  ] {
    harness.set_options(next.sim());
    harness.set_state(particles.clone());
    assert_close(&harness.run(1), &reference_step(&spec, &particles, next, WORLD), &format!("{next:?}"));
  }
}

#[test]
fn many_steps_keep_invariants() {
  let options = Mode { average_forces: true, overlap: Overlap::AdaptiveRange };
  let mut rng = StdRng::seed_from_u64(11);
  let spec = spec(&mut rng, 3);
  // Gęsty stan: ponad 512 kandydatów w sąsiedztwie, więc działa próbkowanie.
  let particles = random_particles(&mut rng, 20_000, 3, false);
  let Some(mut harness) = Harness::new(options.sim(), spec, particles, WORLD) else { return };
  assert_invariants(&harness.run(40), WORLD, "reguły domyślne, 40 kroków");
}

/// Zmiana `ParticleSpec` w trakcie (macierz i parametry reguł) trafia do buforów od następnego
/// kroku, bez rekompilacji i bez nowego `GpuSim`.
#[test]
fn spec_changes_at_runtime() {
  let options = Mode { average_forces: true, overlap: Overlap::Spring };
  let mut rng = StdRng::seed_from_u64(31);
  let first = spec(&mut rng, 4);
  let particles = random_particles(&mut rng, 700, 4, false);
  let Some(mut harness) = Harness::new(options.sim(), first.clone(), particles.clone(), WORLD) else { return };
  assert_close(&harness.run(1), &reference_step(&first, &particles, options, WORLD), "start");

  let mut second = spec(&mut rng, 4);
  second.params.friction = 0.2;
  second.params.repulsion_strength = 40.0;
  second.params.kernel_middle = 25.0;
  second.params.kernel_width = 8.0;
  harness.set_spec(second.clone());
  harness.set_state(particles.clone());
  let actual = harness.run(1);
  assert_close(&actual, &reference_step(&second, &particles, options, WORLD), "po zmianie spec");
  assert_ne!(
    actual[0].pos,
    reference_step(&first, &particles, options, WORLD)[0].pos,
    "zmiana powinna mieć wpływ na wynik"
  );
}
