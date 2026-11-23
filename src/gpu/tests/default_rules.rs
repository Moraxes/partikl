use super::*;

/// Jeden krok fizyki wprost w f64, po wszystkich parach, z odległością na torusie.
/// Ta sama fizyka co `rules/default_rules.wesl`, bez próbkowania sąsiadów; parametry z `spec.params`.
fn reference_step(spec: &ParticleSpec, state: &State, world: Vec2) -> State {
  let particles = &state.particles;
  let u = &spec.params;
  let wrap = |d: f64, size: f64| {
    if d > size / 2.0 {
      d - size
    } else if d < -size / 2.0 {
      d + size
    } else {
      d
    }
  };
  let dt = u.dt as f64;
  let (next, next_model): (Vec<_>, Vec<_>) = particles
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
        let reach = u.repulsion_radius as f64;
        let coeff =
          spec.interactions[other.species as usize].force_coeffs[me.species as usize] as f64;
        let attraction =
          coeff * (1.0 - ((d - u.kernel_middle as f64) / u.kernel_width as f64).abs()).max(0.0);
        if d < reach {
          cx += 0.5 * (d - reach) * dx / d;
          cy += 0.5 * (d - reach) * dy / d;
          cn += 1.0;
        }
        fx += attraction * dx / d;
        fy += attraction * dy / d;
        count += 1.0;
      }
      if count > 0.0 {
        let scale = (u.average_neighbours as f64 / count).min(1.0);
        fx *= scale;
        fy *= scale;
      }
      let (x, y) = (px, py);
      let (mut velx, mut vely) = (me.vel[0] as f64, me.vel[1] as f64);
      if cn > 0.0 {
        velx += cx / cn / dt;
        vely += cy / cn / dt;
      }
      if (velx * velx + vely * vely) < u.velocity_threshold as f64 {
        velx = 0.0;
        vely = 0.0;
      }
      let (freex, freey) = (velx + fx * dt, vely + fy * dt);
      let damping = 1.0 + dt * u.friction as f64 * (freex * freex + freey * freey).sqrt();
      let (mut nx, mut ny) = (x + freex / damping * dt, y + freey / damping * dt);
      for (n, size) in [(&mut nx, world.x as f64), (&mut ny, world.y as f64)] {
        if *n > size / 2.0 {
          *n -= size;
        } else if *n < -size / 2.0 {
          *n += size;
        }
      }

      (
        GpuParticle {
          pos: [nx as f32, ny as f32],
          vel: [(freex / damping) as f32, (freey / damping) as f32],
          species: me.species,
          id: me.id,
        },
        state.model[i],
      )
    })
    .unzip();
  State {
    particles: next,
    model: next_model,
  }
}

#[test]
fn one_step_matches_reference() {
  {
    let mut rng = StdRng::seed_from_u64(7);
    let spec = spec(&mut rng, 4);
    // Mało kandydatów w sąsiedztwie (poniżej progu próbkowania), ale dość gęsto na pary.
    let state = State::new(random_particles(&mut rng, 700, 4));
    let expected = reference_step(&spec, &state, WORLD);
    let Some(mut harness) = harness_for(SimOptions::default(), spec, &state, WORLD) else {
      return;
    };
    assert_close(&run_state(&mut harness, 1), &expected, "jeden krok");
  }
}

/// Świat z więcej niż `MAX_CELLS` komórkami o bazowym boku: większe komórki dają ten sam wynik.
#[test]
fn large_world_matches_reference() {
  let world = Vec2::new(7680.0, 4320.0);
  assert!(Geometry::new(world).cell_size.x > INTERACTION_RADIUS / GRID_REACH as f32);
  let mut rng = StdRng::seed_from_u64(3);
  let spec = spec(&mut rng, 3);
  // Gęsto w pasie, żeby było dość par w zasięgu, i rzadko poza nim.
  let mut particles = initial_particles(3000, 3, world, &mut rng);
  for p in particles.iter_mut().take(2000) {
    p.pos[1] *= 0.02;
    p.vel[1] = 0.0;
  }
  let state = State::new(particles);
  let expected = reference_step(&spec, &state, world);
  let Some(mut harness) = harness_for(SimOptions::default(), spec, &state, world) else {
    return;
  };
  assert_close(&run_state(&mut harness, 1), &expected, "duży świat");
}

/// Krok zgodny z wyrocznią dla różnej liczby typów: shader czyta współczynniki sił inaczej dla
/// do 4 typów (`vec4`), do 8 (dwa `vec4`) i więcej (macierz z bufora).
#[test]
fn one_step_matches_reference_for_any_number_of_types() {
  for types in [1, 2, 4, 5, 8, 9, 13] {
    let mut rng = StdRng::seed_from_u64(40 + types as u64);
    let spec = spec(&mut rng, types);
    let state = State::new(random_particles(&mut rng, 700, types));
    let expected = reference_step(&spec, &state, WORLD);
    let Some(mut harness) = harness_for(SimOptions::default(), spec, &state, WORLD) else {
      return;
    };
    assert_close(
      &run_state(&mut harness, 1),
      &expected,
      &format!("{types} typów"),
    );
  }
}

#[test]
fn many_steps_keep_invariants() {
  let mut rng = StdRng::seed_from_u64(11);
  let spec = spec(&mut rng, 3);
  // Gęsty stan: ponad 512 kandydatów w sąsiedztwie, więc działa próbkowanie.
  let state = State::new(random_particles(&mut rng, 20_000, 3));
  let Some(mut harness) = harness_for(SimOptions::default(), spec, &state, WORLD) else {
    return;
  };
  let after = run_state(&mut harness, 40);
  assert_invariants(&after.particles, WORLD, "reguły domyślne, 40 kroków");
}

/// Histogram następnego kroku liczą siły poprzedniego kroku, więc podmiana stanu między krokami
/// (cząstki w innych komórkach) wymaga przeliczenia od zera: po scatterze nic nie ginie ani się nie powtarza.
#[test]
fn swapping_state_between_steps_keeps_every_particle() {
  let mut rng = StdRng::seed_from_u64(41);
  let spec = spec(&mut rng, 3);
  let first = State::new(random_particles(&mut rng, 5_000, 3));
  let Some(mut harness) = harness_for(SimOptions::default(), spec, &first, WORLD) else {
    return;
  };
  assert_invariants(
    &run_state(&mut harness, 3).particles,
    WORLD,
    "przed podmianą",
  );
  for round in 0..3 {
    let next = State::new(random_particles(&mut rng, 5_000, 3));
    reset(&mut harness, &next);
    assert_invariants(
      &run_state(&mut harness, 2).particles,
      WORLD,
      &format!("po podmianie {round}"),
    );
  }
}

/// Zmiana `ParticleSpec` w trakcie (macierz i parametry reguł) trafia do buforów od następnego
/// kroku, bez rekompilacji i bez nowego `GpuSim`.
#[test]
fn spec_changes_at_runtime() {
  let mut rng = StdRng::seed_from_u64(31);
  let first = spec(&mut rng, 4);
  let state = State::new(random_particles(&mut rng, 700, 4));
  let Some(mut harness) = harness_for(SimOptions::default(), first.clone(), &state, WORLD) else {
    return;
  };
  assert_close(
    &run_state(&mut harness, 1),
    &reference_step(&first, &state, WORLD),
    "start",
  );

  let mut second = spec(&mut rng, 4);
  second.params.friction = 0.2;
  second.params.repulsion_radius = 2.0;
  second.params.kernel_middle = 25.0;
  second.params.kernel_width = 8.0;
  harness.set_spec(second.clone());
  reset(&mut harness, &state);
  let actual = run_state(&mut harness, 1);
  assert_close(
    &actual,
    &reference_step(&second, &state, WORLD),
    "po zmianie spec",
  );
  assert_ne!(
    actual.particles[0].pos,
    reference_step(&first, &state, WORLD).particles[0].pos,
    "zmiana powinna mieć wpływ na wynik"
  );
}
