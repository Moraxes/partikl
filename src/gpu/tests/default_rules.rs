use super::*;

/// Konwersja `f32` -> bity IEEE 754 binary16 (zaokrąglenie do najbliższej parzystej),
/// odpowiadająca `pack2x16float` w `rules/default_rules.wesl`.
fn f32_to_f16_bits(x: f32) -> u16 {
  let bits = x.to_bits();
  let sign = ((bits >> 16) & 0x8000) as u16;
  let exp = ((bits >> 23) & 0xFF) as i32;
  let mant = bits & 0x007F_FFFF;

  if exp == 255 {
    return if mant == 0 {
      sign | 0x7C00
    } else {
      sign | 0x7E00 | ((mant >> 13) as u16)
    };
  }
  if exp > 142 {
    return sign | 0x7C00;
  }
  if exp >= 113 {
    let h_exp = (exp - 112) as u32;
    let h_mant = mant >> 13;
    let rem = mant & 0x1FFF;
    let round_up = rem > 0x1000 || (rem == 0x1000 && (h_mant & 1) != 0);
    let body = ((h_exp << 10) | h_mant) + u32::from(round_up);
    return if body >= 0x7C00 {
      sign | 0x7C00
    } else {
      sign | (body as u16)
    };
  }
  if exp < 102 {
    return sign;
  }
  let full_mant = 0x0080_0000 | mant;
  let shift = (126 - exp) as u32;
  let h_mant = full_mant >> shift;
  let halfway = 1u32 << (shift - 1);
  let rem = full_mant & ((1u32 << shift) - 1);
  let round_up = rem > halfway || (rem == halfway && (h_mant & 1) != 0);
  sign | ((h_mant + u32::from(round_up)) as u16)
}

/// Konwersja bitów IEEE 754 binary16 -> `f32`, odpowiadająca `unpack2x16float`.
fn f16_bits_to_f32(h: u16) -> f32 {
  let h = h as u32;
  let sign = (h & 0x8000) << 16;
  let exp = (h >> 10) & 0x1F;
  let mant = h & 0x03FF;
  if exp == 0 {
    if mant == 0 {
      return f32::from_bits(sign);
    }
    let shift = mant.leading_zeros() - 21;
    let f_exp = 113 - shift;
    let f_mant = (mant << shift) & 0x03FF;
    return f32::from_bits(sign | (f_exp << 23) | (f_mant << 13));
  }
  if exp == 31 {
    return if mant == 0 {
      f32::from_bits(sign | 0x7F80_0000)
    } else {
      f32::from_bits(sign | 0x7FC0_0000 | (mant << 13))
    };
  }
  f32::from_bits(sign | ((exp + 112) << 23) | (mant << 13))
}

/// Kwantyzacja prędkości sąsiada przez `pack2x16float` / `unpack2x16float` w buforze sąsiadów.
/// Specyfikacja WGSL dopuszcza zaokrąglenie do najbliższej parzystej (RNE) lub ku zeru (RTZ);
/// przy typowych prędkościach (`<= 125`, ULP f16 `<= 0.0625`) i `dt = 0.01` różnica między tymi
/// trybami nie przekracza `6.25e-4` i mieści się w tolerancji `1e-3` funkcji `assert_close`.
pub(super) fn quantize_f16(x: f32) -> f64 {
  f16_bits_to_f32(f32_to_f16_bits(x)) as f64
}

#[test]
fn many_steps_keep_invariants() {
  let mut rng = StdRng::seed_from_u64(11);
  let spec = spec(&mut rng, 3);
  // Gęsty stan: ponad 512 kandydatów w sąsiedztwie, więc działa próbkowanie.
  let particles = random_particles(&mut rng, 20_000, 3);
  let Some(mut harness) = Harness::new(SimOptions::default(), spec, particles, WORLD) else {
    return;
  };
  let after = harness.run(40);
  assert_invariants(&after, WORLD, "reguły domyślne, 40 kroków");
}

/// Histogram następnego kroku liczą siły poprzedniego kroku, więc podmiana stanu między krokami
/// (cząstki w innych komórkach) wymaga przeliczenia od zera: po scatterze nic nie ginie ani się nie powtarza.
#[test]
fn swapping_state_between_steps_keeps_every_particle() {
  let mut rng = StdRng::seed_from_u64(41);
  let spec = spec(&mut rng, 3);
  let first = random_particles(&mut rng, 5_000, 3);
  let Some(mut harness) = Harness::new(SimOptions::default(), spec, first, WORLD) else {
    return;
  };
  assert_invariants(&harness.run(3), WORLD, "przed podmianą");
  for round in 0..3 {
    let next = random_particles(&mut rng, 5_000, 3);
    harness.set_state(next);
    assert_invariants(&harness.run(2), WORLD, &format!("po podmianie {round}"));
  }
}

/// Zmiana `ParticleSpec` w trakcie (macierz i parametry reguł) trafia do buforów od następnego
/// kroku, bez rekompilacji i bez nowego `GpuSim`: wynik jak w nowej symulacji z drugą specyfikacją.
#[test]
fn spec_changes_at_runtime() {
  let mut rng = StdRng::seed_from_u64(31);
  let first = spec(&mut rng, 4);
  let particles = random_particles(&mut rng, 700, 4);
  let mut second = spec(&mut rng, 4);
  second.params.friction = 0.2;
  second.params.repulsion_radius = 2.0;
  second.params.kernel_middle = 25.0;
  second.params.kernel_width = 8.0;
  let Some(mut fresh) = Harness::new(
    SimOptions::default(),
    second.clone(),
    particles.clone(),
    WORLD,
  ) else {
    return;
  };
  let expected = fresh.run(1);
  drop(fresh);
  let mut harness = Harness::new(SimOptions::default(), first, particles.clone(), WORLD).unwrap();
  let before = harness.run(1);
  harness.set_spec(second);
  harness.set_state(particles);
  let actual = harness.run(1);
  assert_close(&actual, &expected, "po zmianie spec");
  assert_ne!(
    actual[0].vel, before[0].vel,
    "zmiana powinna mieć wpływ na wynik"
  );
}

fn particle(id: u32, pos: [f32; 2], vel: [f32; 2]) -> GpuParticle {
  GpuParticle {
    pos,
    vel,
    species: 0,
    id,
  }
}

fn isolated_spec(types: usize) -> ParticleSpec {
  let mut spec = ParticleSpec::new(vec![
    Interaction {
      force_coeffs: vec![0.0; types],
    };
    types
  ]);
  spec.params.repulsion_radius = 0.0;
  spec.params.friction = 0.0;
  spec.params.velocity_threshold = 0.0;
  spec.params.separation_stiffness = 0.0;
  spec.params.alignment_rate = 0.0;
  spec.params.cohesion_stiffness = 0.0;
  spec.params.drive_rate = 0.0;
  spec
}

fn speed(p: &GpuParticle) -> f32 {
  (p.vel[0] * p.vel[0] + p.vel[1] * p.vel[1]).sqrt()
}

/// Jeden krok z danymi opcjami (`None` bez GPU).
fn one_step(
  options: SimOptions,
  spec: ParticleSpec,
  particles: Vec<GpuParticle>,
) -> Option<Vec<GpuParticle>> {
  Some(Harness::new(options, spec, particles, WORLD)?.run(1))
}

#[test]
fn f16_quantization_matches_ieee754_binary16() {
  assert_eq!(f32_to_f16_bits(0.0), 0x0000);
  assert_eq!(f32_to_f16_bits(-0.0), 0x8000);
  assert_eq!(f16_bits_to_f32(0x8000).to_bits(), (-0.0f32).to_bits());
  assert_eq!(f32_to_f16_bits(1.0), 0x3C00);
  assert_eq!(f32_to_f16_bits(-2.0), 0xC000);
  assert_eq!(quantize_f16(123.456), 123.4375);

  // Tie-to-even w zakresie normalnym: 1.0 + 0.5 ULP -> 1.0, 1.0 + 1.5 ULP -> 1.0 + 2 ULP.
  assert_eq!(f32_to_f16_bits(1.0 + 2.0f32.powi(-11)), 0x3C00);
  assert_eq!(f32_to_f16_bits(1.0 + 3.0 * 2.0f32.powi(-11)), 0x3C02);

  // Subnormalne i przejście do najmniejszej liczby normalnej (2^-14).
  assert_eq!(f32_to_f16_bits(2.0f32.powi(-25)), 0x0000);
  assert_eq!(f32_to_f16_bits(2.0f32.powi(-25) + 2.0f32.powi(-37)), 0x0001);
  assert_eq!(f32_to_f16_bits(3.0 * 2.0f32.powi(-25)), 0x0002);
  assert_eq!(f32_to_f16_bits(1023.5 * 2.0f32.powi(-24)), 0x0400);
  assert_eq!(f16_bits_to_f32(0x0001), 2.0f32.powi(-24));

  // Maksymalna wartość skończona i przepełnienie.
  assert_eq!(f32_to_f16_bits(65504.0), 0x7BFF);
  assert_eq!(f16_bits_to_f32(0x7BFF), 65504.0);
  assert_eq!(f32_to_f16_bits(65520.0), 0x7C00);
  assert_eq!(f32_to_f16_bits(-65520.0), 0xFC00);
}

/// Separacja odpycha sąsiadów każdego gatunku bliżej niż `separation_radius` (sprężyna: bliżej =
/// mocniej), dalszych nie rusza, a w tłumie rośnie wolniej niż liczba sąsiadów (nasycenie).
#[test]
fn separation_pushes_close_neighbours_of_any_species_apart() {
  let mut spec = isolated_spec(2);
  spec.params.separation_radius = 3.5;
  spec.params.separation_stiffness = 20.0;
  spec.params.separation_saturation = 4.0;
  let mut particles = vec![
    // Różne gatunki, d = 2.
    particle(0, [0.0, 0.0], [0.0, 0.0]),
    particle(1, [2.0, 0.0], [0.0, 0.0]),
    // d = 1: mocniej niż przy d = 2.
    particle(2, [100.0, 0.0], [0.0, 0.0]),
    particle(3, [101.0, 0.0], [0.0, 0.0]),
    // Poza zasięgiem separacji, w zasięgu oddziaływania.
    particle(4, [-100.0, 0.0], [0.0, 0.0]),
    particle(5, [-95.0, 0.0], [0.0, 0.0]),
  ];
  particles[1].species = 1;
  // Ośmiu sąsiadów w jednym miejscu (d = 2) obok cząstki 6.
  particles.push(particle(6, [0.0, 100.0], [0.0, 0.0]));
  for id in 7..15 {
    particles.push(particle(id, [2.0, 100.0], [0.0, 0.0]));
  }
  let Some(actual) = one_step(SimOptions::default(), spec, particles) else {
    return;
  };
  assert!(
    actual[0].vel[0] < 0.0 && actual[1].vel[0] > 0.0,
    "{actual:?}"
  );
  assert!(actual[0].vel[1].abs() < 1e-6);
  assert!(
    actual[3].vel[0] > actual[1].vel[0],
    "d = 1 mocniej niż d = 2"
  );
  assert_eq!(actual[4].vel, [0.0, 0.0]);
  assert_eq!(actual[5].vel, [0.0, 0.0]);
  let ratio = actual[6].vel[0] / actual[0].vel[0];
  assert!(
    ratio > 1.5 && ratio < 8.0,
    "nasycenie: 8 sąsiadów daje {ratio}× jednego"
  );
}

/// Wyrównanie przybliża prędkość do średniej sąsiadów własnego gatunku w `vision_radius`; inne
/// gatunki i dalsi sąsiedzi nie wpływają.
#[test]
fn alignment_pulls_velocity_toward_own_species_neighbours() {
  let mut spec = isolated_spec(2);
  spec.params.vision_radius = 25.0;
  spec.params.alignment_rate = 20.0;
  let mut particles = vec![
    particle(0, [0.0, 0.0], [10.0, 0.0]),
    particle(1, [10.0, 0.0], [10.0, 20.0]),
    particle(2, [0.0, 10.0], [10.0, -4.0]),
    // Para poza vision_radius = 25.0 (w INTERACTION_RADIUS = 40.0).
    particle(3, [100.0, 0.0], [5.0, 0.0]),
    particle(4, [130.0, 0.0], [5.0, 50.0]),
    // Inny gatunek obok: brak wpływu.
    particle(5, [-100.0, 0.0], [5.0, 0.0]),
    particle(6, [-95.0, 0.0], [5.0, 50.0]),
  ];
  particles[6].species = 1;
  let Some(actual) = one_step(SimOptions::default(), spec, particles) else {
    return;
  };
  assert!(actual[0].vel[1] > 1.0);
  assert!((actual[0].vel[0] - 10.0).abs() < 1e-4);
  assert_eq!(actual[3].vel, [5.0, 0.0]);
  assert_eq!(actual[4].vel, [5.0, 50.0]);
  assert_eq!(actual[5].vel, [5.0, 0.0]);
}

/// Spójność ciągnie do środka sąsiadów własnego gatunku w `vision_radius`, także przez szew torusa.
#[test]
fn cohesion_pulls_toward_neighbours_within_vision_radius() {
  let mut spec = isolated_spec(1);
  spec.params.vision_radius = 25.0;
  spec.params.cohesion_stiffness = 2.0;
  let particles = vec![
    particle(0, [0.0, 0.0], [0.0, 0.0]),
    particle(1, [10.0, 0.0], [0.0, 0.0]),
    particle(2, [0.0, 20.0], [0.0, 0.0]),
    // Poza vision_radius = 25.0, choć wewnątrz INTERACTION_RADIUS = 40.0.
    particle(3, [100.0, 0.0], [0.0, 0.0]),
    particle(4, [130.0, 0.0], [0.0, 0.0]),
    // Para przez szew torusa (WORLD.x = 500): odległość 10.
    particle(5, [-245.0, -100.0], [0.0, 0.0]),
    particle(6, [245.0, -100.0], [0.0, 0.0]),
  ];
  let Some(actual) = one_step(SimOptions::default(), spec, particles) else {
    return;
  };
  assert!(actual[0].vel[0] > 0.0 && actual[0].vel[1] > 0.0);
  assert_eq!(actual[3].vel, [0.0, 0.0]);
  assert_eq!(actual[4].vel, [0.0, 0.0]);
  assert!(actual[5].vel[0] < 0.0 && actual[6].vel[0] > 0.0);
}

/// Wszystkie kombinacje opcji reguł (opis, opcje).
fn option_combinations() -> Vec<(String, SimOptions)> {
  let mut out = Vec::new();
  for overlap in ["unweighted", "weighted"] {
    for damp in ["off", "force"] {
      let options = SimOptions::default()
        .with("overlap", overlap)
        .with("overlap_damp", damp);
      out.push((format!("overlap={overlap} overlap_damp={damp}"), options));
    }
  }
  out
}

/// Dodatni współczynnik macierzy przyciąga sąsiada w środku jądra, ujemny odpycha; w każdym
/// wariancie opcji.
#[test]
fn attraction_follows_matrix_sign_in_every_option() {
  let mut spec = isolated_spec(2);
  // Typ 0 przyciąga typ 1 (`M[0][1] > 0`), typ 1 odpycha typ 0.
  spec.interactions[0].force_coeffs[1] = 100.0;
  spec.interactions[1].force_coeffs[0] = -100.0;
  let mut particles = vec![
    particle(0, [0.0, 0.0], [0.0, 0.0]),
    particle(1, [30.0, 0.0], [0.0, 0.0]),
  ];
  particles[0].species = 0;
  particles[1].species = 1;
  for (ctx, options) in option_combinations() {
    let Some(actual) = one_step(options, spec.clone(), particles.clone()) else {
      return;
    };
    let ctx = format!("{ctx}: {actual:?}");
    // Cząstka 1 (typ 1) jest przyciągana przez typ 0: w stronę cząstki 0.
    assert!(actual[1].vel[0] < 0.0, "{ctx}");
    // Cząstka 0 (typ 0) jest odpychana przez typ 1: od cząstki 1.
    assert!(actual[0].vel[0] < 0.0, "{ctx}");
  }
}

/// Każda kombinacja opcji kompiluje się i trzyma niezmienniki na gęstym stanie, a warianty
/// dają różne wyniki.
#[test]
fn every_option_combination_runs_and_differs() {
  let mut rng = StdRng::seed_from_u64(77);
  let spec = spec(&mut rng, 3);
  let particles = random_particles(&mut rng, 5_000, 3);
  let mut results = Vec::new();
  for (ctx, options) in option_combinations() {
    let Some(mut harness) = Harness::new(options, spec.clone(), particles.clone(), WORLD) else {
      return;
    };
    let after = harness.run(20);
    assert_invariants(&after, WORLD, &ctx);
    results.push(after);
  }
  for i in 0..results.len() {
    for j in i + 1..results.len() {
      assert!(
        results[i]
          .iter()
          .zip(&results[j])
          .any(|(a, b)| (a.vel[0] - b.vel[0]).abs() > 1e-2),
        "warianty {i} i {j} dają ten sam wynik"
      );
    }
  }
}

#[test]
fn drive_only_scales_speed_toward_target_speed() {
  let mut spec = isolated_spec(1);
  spec.params.target_speed = 40.0;
  spec.params.drive_rate = 5.0;
  let particles = vec![
    particle(0, [-200.0, 0.0], [6.0, 8.0]),
    particle(1, [-100.0, 0.0], [60.0, 0.0]),
    particle(2, [0.0, 0.0], [40.0, 0.0]),
    particle(3, [100.0, 0.0], [0.0, 0.0]),
    // Prędkość poniżej 1e-3: drive nie działa.
    particle(4, [200.0, 0.0], [0.0005, 0.0]),
    particle(5, [0.0, 100.0], [-30.0, 40.0]),
  ];
  let Some(actual) = one_step(SimOptions::default(), spec, particles) else {
    return;
  };
  assert!(speed(&actual[0]) > 10.0);
  assert!((actual[0].vel[1] / actual[0].vel[0] - 8.0 / 6.0).abs() < 1e-5);
  assert!(speed(&actual[1]) < 60.0);
  assert_eq!(actual[1].vel[1], 0.0);
  assert!((actual[2].vel[0] - 40.0).abs() < 1e-5);
  assert_eq!(actual[3].vel, [0.0, 0.0]);
  assert!((actual[4].vel[0] - 0.0005).abs() < 1e-6);
  assert!(speed(&actual[5]) < 50.0);
  assert!((actual[5].vel[1] / actual[5].vel[0] + 40.0 / 30.0).abs() < 1e-5);
}

/// Przy zerowych siłach boidów ich promienie nie zmieniają wyniku, a domyślne boidy tak.
#[test]
fn zero_boid_strengths_ignore_boid_radii() {
  let mut rng = StdRng::seed_from_u64(19);
  let default_spec = spec(&mut rng, 4);
  let mut zero_boids = default_spec.clone();
  zero_boids.params.separation_stiffness = 0.0;
  zero_boids.params.alignment_rate = 0.0;
  zero_boids.params.cohesion_stiffness = 0.0;
  zero_boids.params.drive_rate = 0.0;
  let mut no_radii = zero_boids.clone();
  no_radii.params.separation_radius = 0.0;
  no_radii.params.vision_radius = 0.0;
  no_radii.params.target_speed = 0.0;

  let particles = random_particles(&mut rng, 700, 4);
  let Some(actual) = one_step(SimOptions::default(), zero_boids, particles.clone()) else {
    return;
  };
  let actual_no_radii = one_step(SimOptions::default(), no_radii, particles.clone()).unwrap();
  assert_close(
    &actual_no_radii,
    &actual,
    "promienie boidów przy zerowych siłach nie zmieniają wyniku",
  );
  let with_boids = one_step(SimOptions::default(), default_spec, particles).unwrap();
  assert!(
    actual
      .iter()
      .zip(&with_boids)
      .any(|(a, b)| (a.vel[0] - b.vel[0]).abs() > 1e-3),
    "wyzerowane boidy powinny różnić się od domyślnych"
  );
}

#[test]
fn velocity_above_f16_max_does_not_propagate_nan() {
  // Prędkości powyżej f16::MAX (65504) w pack_neighbour są ograniczane przed pack2x16float,
  // więc przeciwnie skierowane szybkie cząstki nie dają ±Inf ani NaN u sąsiadów.
  let mut isolated = isolated_spec(1);
  isolated.params.vision_radius = 40.0;
  isolated.params.alignment_rate = 1.0;
  let one_step = vec![
    particle(0, [0.0, 0.0], [100_000.0, -90_000.0]),
    particle(1, [-10.0, 0.0], [-100_000.0, 90_000.0]),
    particle(2, [5.0, 0.0], [0.0, 0.0]),
    particle(3, [-5.0, 0.0], [10.0, -5.0]),
  ];
  let Some(mut harness) = Harness::new(SimOptions::default(), isolated, one_step.clone(), WORLD)
  else {
    return;
  };
  harness.set_stats(true);
  // Rozgrzewka kompilacji przebiegu statystyk przed właściwym krokiem.
  harness.step(1);
  harness.take_stats();
  harness.set_state(one_step);
  let actual = harness.run(1);
  let stats = harness.take_stats().expect("statystyki po kroku");
  assert_eq!(stats.nan_count, 0);
  assert!(
    actual.iter().all(|p| p.vel.iter().all(|v| v.is_finite())),
    "prędkość > 65504 w jednym kroku: {actual:?}"
  );
  drop(harness);

  // Pełna fizyka domyślna przez kilkadziesiąt kroków z szybkimi cząstkami w tłumie.
  let mut rng = StdRng::seed_from_u64(90);
  let full_spec = spec(&mut rng, 3);
  let mut particles = random_particles(&mut rng, 200, 3);
  particles[0].pos = [0.0, 0.0];
  particles[0].vel = [80_000.0, -80_000.0];
  particles[1].pos = [3.0, 0.0];
  particles[1].vel = [-80_000.0, 80_000.0];
  particles[2].pos = [1.5, 1.0];
  particles[2].vel = [0.0, 0.0];
  let Some(mut multi_harness) =
    Harness::new(SimOptions::default(), full_spec, particles.clone(), WORLD)
  else {
    return;
  };
  multi_harness.set_stats(true);
  multi_harness.step(1);
  multi_harness.take_stats();
  multi_harness.set_state(particles);
  let after = multi_harness.run(30);
  let stats = multi_harness
    .take_stats()
    .expect("statystyki po 30 krokach");
  assert_eq!(stats.nan_count, 0);
  assert_invariants(&after, WORLD, "prędkość > 65504, 30 kroków");
}

#[test]
fn dense_cluster_keeps_zero_nans_for_many_steps() {
  use std::f32::consts::TAU;

  let mut rng = StdRng::seed_from_u64(91);
  let spec = spec(&mut rng, 3);
  // 5000 cząstek ściśniętych w promieniu 5.0 (< separation_radius = 10.0, < vision_radius = 40.0),
  // z dużym próbkowaniem sąsiadów (ponad 256 kandydatów).
  let particles: Vec<GpuParticle> = (0..5_000u32)
    .map(|id| {
      let r = 5.0 * rng.random::<f32>().sqrt();
      let angle = rng.random::<f32>() * TAU;
      GpuParticle {
        pos: [r * angle.cos(), r * angle.sin()],
        vel: [rng.random_range(-50.0..50.0), rng.random_range(-50.0..50.0)],
        species: rng.random_range(0..3),
        id,
      }
    })
    .collect();
  let Some(mut harness) = Harness::new(SimOptions::default(), spec, particles.clone(), WORLD)
  else {
    return;
  };
  harness.set_stats(true);
  harness.step(1);
  harness.take_stats();
  harness.set_state(particles);
  let after = harness.run(60);
  let stats = harness.take_stats().expect("statystyki gęstego skupiska");
  assert_eq!(stats.nan_count, 0);
  assert!(
    stats.max_speed.is_finite(),
    "max_speed niefinite: {}",
    stats.max_speed
  );
  assert!(
    stats.kinetic_energy.is_finite(),
    "energia niefinite: {}",
    stats.kinetic_energy
  );
  assert_invariants(&after, WORLD, "gęste skupisko, 60 kroków");
}
