use super::*;

#[test]
fn layout_matches_wgsl() {
  assert_eq!(size_of::<GpuParticle>(), 24);
  assert_eq!(EngineUniforms::min_size().get(), 56);
  assert_eq!(RuleParams::min_size().get(), 60);
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
      assert!(
        g.cell_size.x * GRID_REACH as f32 >= INTERACTION_RADIUS - 1e-3,
        "{world}"
      );
      assert!(
        g.cell_size.y * GRID_REACH as f32 >= INTERACTION_RADIUS - 1e-3,
        "{world}"
      );
    }
  }
}

#[test]
fn deterministic_mode_repeats_bit_for_bit() {
  let options = SimOptions::default();
  let run = || {
    let mut rng = StdRng::seed_from_u64(5);
    let spec = spec(&mut rng, 3);
    let particles = random_particles(&mut rng, 5_000, 3);
    let mut harness = Harness::new(options.clone(), spec, particles, WORLD)?;
    harness.set_deterministic(true);
    Some(harness.run(32))
  };
  let (Some(first), Some(second)) = (run(), run()) else {
    return;
  };
  assert_eq!(
    first, second,
    "trajektorie różnią się mimo trybu deterministycznego"
  );
}

/// Pętla jak w aplikacji (`real_time`): kroki wynikają ze stałego kroku w czasie rzeczywistym,
/// więc w sekundzie wychodzi ok. 100 kroków przy pełnej prędkości symulacji.
#[test]
fn real_time_loop_runs_the_fixed_step_clock() {
  use crate::gpu::headless::Offscreen;
  let mut rng = StdRng::seed_from_u64(31);
  let spec = spec(&mut rng, 3);
  let particles = random_particles(&mut rng, 2_000, 3);
  let offscreen = Offscreen {
    size: UVec2::new(64, 40),
    quality: RenderQuality::default(),
    app_colors: false,
    real_time: true,
  };
  let Some(mut harness) = Harness::with_render(
    SimOptions::default(),
    spec,
    particles,
    WORLD,
    Some(offscreen),
  ) else {
    return;
  };
  harness.run_loop(std::time::Duration::from_millis(500));
  let stats = harness.run_loop(std::time::Duration::from_secs(1));
  assert!(
    stats.frame_ns.len() > 10,
    "klatek: {}",
    stats.frame_ns.len()
  );
  // 100 kroków na sekundę z zapasem na obciążenie maszyny.
  assert!(
    (50..=150).contains(&stats.steps),
    "kroków w sekundzie: {}",
    stats.steps
  );
  let speed = stats
    .sim_speed
    .last()
    .copied()
    .flatten()
    .expect("prędkość symulacji");
  assert!(speed > 0.5, "prędkość {speed}");
}

/// Średnia prędkość `|v|`
fn mean_speed(particles: &[GpuParticle]) -> f32 {
  let sum: f32 = particles.iter().map(|p| Vec2::from(p.vel).length()).sum();
  sum / particles.len() as f32
}

/// Podkroki (K = 2): bez NaN, te same id i liczba cząstek, a prędkość w granicach szumu względem
/// K = 1 (progi bezpieczeństwa, nie wartości analogowe).
#[test]
fn substeps_keep_particles_finite_and_speed_in_range() {
  let run = |substeps: u32| {
    let mut rng = StdRng::seed_from_u64(41);
    let spec = spec(&mut rng, 3);
    let particles = random_particles(&mut rng, 5_000, 3);
    let mut harness = Harness::new(SimOptions::default(), spec, particles, WORLD)?;
    harness.set_substeps(substeps);
    Some(harness.run(40))
  };
  let (Some(single), Some(double)) = (run(1), run(2)) else {
    return;
  };
  assert_eq!(double.len(), 5_000);
  let ids: Vec<u32> = double.iter().map(|p| p.id).collect();
  assert_eq!(ids, (0..5_000).collect::<Vec<u32>>());
  assert!(
    double
      .iter()
      .all(|p| p.pos.iter().chain(&p.vel).all(|v| v.is_finite())),
    "NaN albo nieskończoność"
  );
  let ratio = mean_speed(&double) / mean_speed(&single);
  assert!(
    (0.5..2.0).contains(&ratio),
    "stosunek prędkości K=2 do K=1: {ratio}"
  );
}

/// Zmiana K nie zmienia prędkości
#[test]
fn changing_substeps_keeps_velocity() {
  let mut rng = StdRng::seed_from_u64(43);
  let spec = spec(&mut rng, 3);
  let particles = random_particles(&mut rng, 5_000, 3);
  let before = mean_speed(&particles);
  let Some(mut harness) = Harness::new(SimOptions::default(), spec, particles, WORLD) else {
    return;
  };
  harness.set_substeps(4);
  let after = mean_speed(&harness.run(1));
  assert!(
    (0.5..2.0).contains(&(after / before)),
    "prędkość po zmianie K: {before} -> {after}"
  );
  // Powrót do K = 1 nie przeskalowuje prędkości.
  harness.set_substeps(1);
  let back = mean_speed(&harness.run(1));
  assert!(
    (0.5..2.0).contains(&(back / before)),
    "prędkość po powrocie do K = 1: {before} -> {back}"
  );
}

/// `GpuProgress` liczy podkroki GPU (takty × K), a `GpuParams::step_count` takty.
#[test]
fn progress_counts_gpu_substeps() {
  let mut rng = StdRng::seed_from_u64(47);
  let spec = spec(&mut rng, 3);
  let particles = random_particles(&mut rng, 1_000, 3);
  let Some(mut harness) = Harness::new(SimOptions::default(), spec, particles, WORLD) else {
    return;
  };
  harness.set_substeps(3);
  harness.step(5);
  assert_eq!(harness.world().resource::<GpuParams>().step_count, 5);
  assert_eq!(
    harness
      .world()
      .resource::<GpuProgress>()
      .0
      .load(std::sync::atomic::Ordering::SeqCst),
    15
  );
}

#[test]
fn set_state_replaces_particles_on_gpu() {
  let mut rng = StdRng::seed_from_u64(9);
  let spec = spec(&mut rng, 3);
  let first = random_particles(&mut rng, 2000, 3);
  let second = random_particles(&mut rng, 2000, 3);
  let Some(mut harness) = Harness::new(SimOptions::default(), spec, first, WORLD) else {
    return;
  };
  harness.run(4);
  harness.set_state(second.clone());
  assert_eq!(harness.run(0), second);
}

#[test]
fn timed_step_reports_every_pass() {
  let mut rng = StdRng::seed_from_u64(13);
  let spec = spec(&mut rng, 3);
  let particles = random_particles(&mut rng, 20_000, 3);
  let Some(mut harness) = Harness::new(SimOptions::default(), spec, particles, WORLD) else {
    return;
  };
  harness.run(4);
  harness.set_gpu_timing(true);
  // Pierwsza klatka z pomiarem bywa odrzucona (zerowy znacznik).
  harness.step(8);
  harness.take_gpu_times();
  harness.step(8);
  let Some(times) = harness.take_gpu_times() else {
    eprintln!("pomijam: brak znaczników czasu GPU");
    return;
  };
  assert_eq!(times.steps, 1);
  for (entry, ns) in &times.per_step_ns {
    // `sort_cells` działa tylko w trybie deterministycznym, a `count` tylko w pierwszym kroku po
    // wgraniu stanu (potem histogram liczą siły).
    if matches!(*entry, "sort_cells" | "count") {
      assert_eq!(*ns, 0.0);
    } else {
      assert!(*ns > 0.0, "{entry}: {ns}");
    }
  }
}

#[test]
fn timed_step_gives_the_same_state() {
  let options = SimOptions::default();
  let run = |timed: bool| {
    let mut rng = StdRng::seed_from_u64(17);
    let spec = spec(&mut rng, 3);
    let particles = random_particles(&mut rng, 5_000, 3);
    let mut harness = Harness::new(options.clone(), spec, particles, WORLD)?;
    harness.set_deterministic(true);
    harness.set_gpu_timing(timed);
    harness.step(MAX_STEPS_PER_FRAME as u64);
    Some(harness.run(0))
  };
  let (Some(plain), Some(timed)) = (run(false), run(true)) else {
    return;
  };
  assert_eq!(plain, timed, "pomiar czasu zmienia wynik kroku");
}

/// Co liczą reguły diagnostyczne dla jednej cząstki: suma wag sąsiadów w zasięgu oddziaływania,
/// suma wag · delta i suma wag · coeff.
#[derive(Clone, Copy, Debug, Default)]
struct Neighbourhood {
  count: f64,
  delta: [f64; 2],
  coeff: f64,
}

/// Wynik reguł diagnostycznych liczony wprost po wszystkich parach w f64 (odległość na torusie).
fn brute_force_neighbourhoods(
  spec: &ParticleSpec,
  particles: &[GpuParticle],
  world: Vec2,
) -> Vec<Neighbourhood> {
  let wrap = |d: f64, size: f64| {
    if d > size / 2.0 {
      d - size
    } else if d < -size / 2.0 {
      d + size
    } else {
      d
    }
  };
  particles
    .iter()
    .enumerate()
    .map(|(i, me)| {
      let mut n = Neighbourhood::default();
      for (j, other) in particles.iter().enumerate() {
        let dx = wrap(other.pos[0] as f64 - me.pos[0] as f64, world.x as f64);
        let dy = wrap(other.pos[1] as f64 - me.pos[1] as f64, world.y as f64);
        let d = (dx * dx + dy * dy).sqrt();
        if j == i || d == 0.0 || d > INTERACTION_RADIUS as f64 {
          continue;
        }
        n.count += 1.0;
        n.delta[0] += dx;
        n.delta[1] += dy;
        n.coeff +=
          spec.interactions[other.species as usize].force_coeffs[me.species as usize] as f64;
      }
      n
    })
    .collect()
}

/// Jeden krok z regułami diagnostycznymi: wynik w `vel` cząstki i w jej modelu (patrz
/// `diagnostic_rules.wesl`).
fn diagnostic_step(
  spec: ParticleSpec,
  particles: Vec<GpuParticle>,
  world: Vec2,
) -> Option<Vec<Neighbourhood>> {
  let mut harness = Harness::with_rules(
    RuleSource::Diagnostic,
    SimOptions::default(),
    spec,
    particles,
    world,
  )?;
  let (particles, model) = harness.run_with_model::<DiagnosticModel>(1);
  Some(
    particles
      .iter()
      .zip(&model)
      .map(|(p, m)| Neighbourhood {
        count: m.count as f64,
        delta: [p.vel[0] as f64, p.vel[1] as f64],
        coeff: m.coeff as f64,
      })
      .collect(),
  )
}

/// `Model` reguł diagnostycznych (`//@field count`, `coeff`).
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct DiagnosticModel {
  count: f32,
  coeff: f32,
}

// ---- Silnik siatki z regułami diagnostycznymi (bez fizyki) ----

/// Wynik reguł diagnostycznych musi zgadzać się z wyrocznią brute force: liczba sąsiadów co do
/// jednego (bez próbkowania), sumy delta i coeff z tolerancją na f32.
fn assert_neighbourhoods(actual: &[Neighbourhood], expected: &[Neighbourhood], context: &str) {
  assert_eq!(actual.len(), expected.len(), "{context}");
  for (i, (a, e)) in actual.iter().zip(expected).enumerate() {
    assert_eq!(a.count, e.count, "{context}: cząstka {i}: liczba sąsiadów");
    for k in 0..2 {
      assert!(
        (a.delta[k] - e.delta[k]).abs() <= 5e-3 + 1e-5 * e.delta[k].abs(),
        "{context}: cząstka {i}: suma delta {:?} wobec {:?}",
        a.delta,
        e.delta
      );
    }
    assert!(
      (a.coeff - e.coeff).abs() <= 1e-2 + 1e-3 * e.coeff.abs(),
      "{context}: cząstka {i}: suma coeff {} wobec {}",
      a.coeff,
      e.coeff
    );
  }
}

#[test]
fn engine_finds_every_neighbour_in_range() {
  let mut rng = StdRng::seed_from_u64(50);
  let spec = spec(&mut rng, 3);
  let particles = random_particles(&mut rng, 700, 3);
  let expected = brute_force_neighbourhoods(&spec, &particles, WORLD);
  let Some(actual) = diagnostic_step(spec, particles, WORLD) else {
    return;
  };
  assert!(
    expected.iter().any(|n| n.count > 10.0),
    "scena bez sąsiadów nic nie sprawdza"
  );
  assert_neighbourhoods(&actual, &expected, "700 cząstek");
}

/// Współczynnik pary czytany z macierzy sił: shader robi to inaczej dla do 4 typów (`vec4`), do 8
/// (dwa `vec4`) i więcej (bufor), a silnik ma przekazać regule ten sam `coeff` w każdym wariancie.
#[test]
fn engine_passes_the_right_coefficient_for_any_number_of_types() {
  for types in [1, 2, 4, 5, 8, 9, 13] {
    let mut rng = StdRng::seed_from_u64(60 + types as u64);
    let spec = spec(&mut rng, types);
    let particles = random_particles(&mut rng, 500, types);
    let expected = brute_force_neighbourhoods(&spec, &particles, WORLD);
    let Some(actual) = diagnostic_step(spec, particles, WORLD) else {
      return;
    };
    assert_neighbourhoods(&actual, &expected, &format!("{types} typów"));
  }
}

/// Sąsiedzi przez szew torusa: cztery cząstki przy narożnikach świata widzą się nawzajem.
#[test]
fn engine_finds_neighbours_across_the_world_seam() {
  let corners = [
    Vec2::new(-249.0, -164.0),
    Vec2::new(249.0, -164.0),
    Vec2::new(-249.0, 164.0),
    Vec2::new(249.0, 164.0),
  ];
  let mut particles: Vec<GpuParticle> = corners
    .iter()
    .enumerate()
    .map(|(id, p)| GpuParticle {
      pos: p.to_array(),
      vel: [0.0, 0.0],
      species: 0,
      id: id as u32,
    })
    .collect();
  // Samotna cząstka w środku, bez sąsiadów.
  particles.push(GpuParticle {
    pos: [0.0, 0.0],
    vel: [0.0, 0.0],
    species: 0,
    id: 4,
  });
  let spec = spec(&mut StdRng::seed_from_u64(1), 2);
  let expected = brute_force_neighbourhoods(&spec, &particles, WORLD);
  let Some(actual) = diagnostic_step(spec, particles, WORLD) else {
    return;
  };
  assert_neighbourhoods(&actual, &expected, "narożniki");
  // Wprost, bez wyroczni: cząstka w (-249, -164) widzi trzy pozostałe po drugiej stronie szwu,
  // o 2 jednostki w osi (i √8 po przekątnej).
  assert_eq!(actual[0].count, 3.0);
  assert!(
    (actual[0].delta[0] + 4.0).abs() < 1e-3 && (actual[0].delta[1] + 4.0).abs() < 1e-3,
    "{:?}",
    actual[0].delta
  );
  assert_eq!(actual[4].count, 0.0);
}

/// Świat z więcej niż `MAX_CELLS` komórkami o bazowym boku ma większe komórki; wynik ten sam.
#[test]
fn engine_counts_the_same_with_bigger_cells_in_a_large_world() {
  let world = Vec2::new(7680.0, 4320.0);
  assert!(Geometry::new(world).cell_size.x > INTERACTION_RADIUS / GRID_REACH as f32);
  let mut rng = StdRng::seed_from_u64(70);
  let spec = spec(&mut rng, 3);
  // Gęsto w pasie, żeby było dość par w zasięgu, i rzadko poza nim.
  let mut particles = initial_particles(3000, 3, world, &mut rng);
  for p in particles.iter_mut().take(2000) {
    p.pos[1] *= 0.02;
  }
  let expected = brute_force_neighbourhoods(&spec, &particles, world);
  let Some(actual) = diagnostic_step(spec, particles, world) else {
    return;
  };
  assert_neighbourhoods(&actual, &expected, "duży świat");
}

/// Powyżej progu próbkowania (512 kandydatów) silnik liczy co `every`-tego sąsiada z wagą `every`:
/// suma po wszystkich cząstkach nie ma obciążenia, a błąd pojedynczej cząstki jest ograniczony.
#[test]
fn engine_samples_dense_states_without_bias() {
  let mut rng = StdRng::seed_from_u64(51);
  let spec = spec(&mut rng, 3);
  let particles = random_particles(&mut rng, 20_000, 3);
  let expected = brute_force_neighbourhoods(&spec, &particles, WORLD);
  let Some(actual) = diagnostic_step(spec, particles, WORLD) else {
    return;
  };
  let (sum_actual, sum_expected): (f64, f64) = (
    actual.iter().map(|n| n.count).sum(),
    expected.iter().map(|n| n.count).sum(),
  );
  // Średnio ponad 500 sąsiadów, czyli próbkowanie rzeczywiście działa.
  assert!(
    sum_expected / expected.len() as f64 > 500.0,
    "stan za rzadki: {}",
    sum_expected / expected.len() as f64
  );
  assert!(
    (sum_actual / sum_expected - 1.0).abs() < 0.01,
    "suma sąsiadów {sum_actual} wobec {sum_expected}"
  );
  for (i, (a, e)) in actual.iter().zip(&expected).enumerate() {
    assert!(
      (a.count - e.count).abs() <= 0.25 * e.count.max(10.0),
      "cząstka {i}: oszacowano {} sąsiadów, jest {}",
      a.count,
      e.count
    );
  }
}

/// Próg próbkowania zmieniany w trakcie (klawisz C): dla każdego progu kroki nie dają NaN i stan
/// zachowuje wszystkie cząstki (te same id, ta sama liczba).
#[test]
fn sampling_cap_keeps_every_particle_finite_and_present() {
  let mut rng = StdRng::seed_from_u64(77);
  let spec = spec(&mut rng, 3);
  let particles = random_particles(&mut rng, 20_000, 3);
  let mut ids: Vec<u32> = particles.iter().map(|p| p.id).collect();
  ids.sort_unstable();
  let Some(mut harness) = Harness::new(SimOptions::default(), spec, particles, WORLD) else {
    return;
  };
  for cap in [128, 256, 512, 1024] {
    harness.set_max_candidates(cap);
    harness.run(3);
    let state = harness.run(0);
    assert_eq!(state.len(), ids.len(), "próg {cap}");
    assert!(
      state
        .iter()
        .all(|p| p.pos.iter().chain(&p.vel).all(|v| v.is_finite())),
      "próg {cap}: nieskończone położenie"
    );
    let mut got: Vec<u32> = state.iter().map(|p| p.id).collect();
    got.sort_unstable();
    assert_eq!(got, ids, "próg {cap}: id cząstek");
  }
}

/// Niezmienniki stanu dla dowolnych reguł: te same sprawdzenia co dla reguł domyślnych, tu dla
/// diagnostycznych (nic się nie rusza, ale silnik nie może gubić ani dublować cząstek).
#[test]
fn state_invariants_hold_for_the_diagnostic_rules() {
  let mut rng = StdRng::seed_from_u64(80);
  let spec = spec(&mut rng, 3);
  let particles = random_particles(&mut rng, 5_000, 3);
  let Some(mut harness) = Harness::with_rules(
    RuleSource::Diagnostic,
    SimOptions::default(),
    spec,
    particles,
    WORLD,
  ) else {
    return;
  };
  assert_invariants(&harness.run(10), WORLD, "reguły diagnostyczne, 10 kroków");
}

/// Model cząstki (pola reguł) przenosi się razem z nią przez scatter (i sortowanie komórek w
/// trybie deterministycznym): reguły nie zmieniają modelu, więc po wielu krokach każda cząstka
/// ma nadal swoją wartość.
#[test]
fn model_stays_with_its_particle() {
  for deterministic in [false, true] {
    let mut rng = StdRng::seed_from_u64(95);
    let spec = spec(&mut rng, 3);
    let mut state = State::new(random_particles(&mut rng, 6_000, 3));
    state.model = (0..state.particles.len()).map(|id| id as f32).collect();
    let Some(mut harness) = harness_for(SimOptions::default(), spec, &state, WORLD) else {
      return;
    };
    harness.set_deterministic(deterministic);
    let after = run_state(&mut harness, 20);
    assert_eq!(
      after.model, state.model,
      "model rozjechał się z cząstkami (deterministic = {deterministic})"
    );
  }
}
