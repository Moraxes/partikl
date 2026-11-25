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

/// Jeden krok fizyki wprost w f64, po wszystkich parach, z odległością na torusie.
/// Ta sama fizyka co `rules/default_rules.wesl` i `rules/nodes.wesl`, bez próbkowania sąsiadów;
/// parametry z `spec.params`, z ograniczeniem składowych prędkości sąsiada do `[-65504, 65504]`
/// i kwantyzacją f16 (`pack2x16float`/`unpack2x16float`).
fn reference_step(spec: &ParticleSpec, particles: &[GpuParticle], world: Vec2) -> Vec<GpuParticle> {
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
  particles
    .iter()
    .enumerate()
    .map(|(i, me)| {
      let (px, py) = (me.pos[0] as f64, me.pos[1] as f64);
      let (mut fx, mut fy, mut count) = (0.0f64, 0.0f64, 0.0f64);
      let (mut cx, mut cy, mut cn) = (0.0f64, 0.0f64, 0.0f64);
      let (mut sx, mut sy, mut sep_n) = (0.0f64, 0.0f64, 0.0f64);
      let (mut ax, mut ay, mut align_w) = (0.0f64, 0.0f64, 0.0f64);
      let (mut cohx, mut cohy, mut view_n) = (0.0f64, 0.0f64, 0.0f64);
      for (j, other) in particles.iter().enumerate() {
        let dx = wrap(other.pos[0] as f64 - px, world.x as f64);
        let dy = wrap(other.pos[1] as f64 - py, world.y as f64);
        let d = (dx * dx + dy * dy).sqrt();
        if j == i || d == 0.0 || d > INTERACTION_RADIUS as f64 {
          continue;
        }
        let (dirx, diry) = (dx / d, dy / d);
        let reach = u.repulsion_radius as f64;
        let coeff =
          spec.interactions[other.species as usize].force_coeffs[me.species as usize] as f64;
        let attraction =
          coeff * (1.0 - ((d - u.kernel_middle as f64) / u.kernel_width as f64).abs()).max(0.0);
        if d < reach {
          cx += 0.5 * (d - reach) * dirx;
          cy += 0.5 * (d - reach) * diry;
          cn += 1.0;
        }
        fx += attraction * dirx;
        fy += attraction * diry;
        count += 1.0;

        if d < u.separation_radius as f64 {
          let ws = u.separation_weight as f64;
          let sep_d = d.max(1e-3);
          sx += ws * (-dirx / sep_d);
          sy += ws * (-diry / sep_d);
          sep_n += 1.0;
        }
        if d < u.vision_radius as f64 {
          let wa = u.alignment_weight as f64;
          let wc = u.coherence_weight as f64;
          ax += wa * quantize_f16(other.vel[0].clamp(-65504.0, 65504.0));
          ay += wa * quantize_f16(other.vel[1].clamp(-65504.0, 65504.0));
          align_w += wa;
          cohx += wc * dx;
          cohy += wc * dy;
          view_n += 1.0;
        }
      }
      if count > 0.0 {
        let scale = (u.average_neighbours as f64 / count).min(1.0);
        fx *= scale;
        fy *= scale;
      }
      let (sep_x, sep_y) = if sep_n > 0.0 {
        (sx / sep_n, sy / sep_n)
      } else {
        (0.0, 0.0)
      };
      // W `integrate_particle` człon `a_align` odejmuje własną prędkość sprzed poprawki rozsuwania.
      let (self_vx, self_vy) = (me.vel[0] as f64, me.vel[1] as f64);
      let (align_x, align_y, coh_x, coh_y) = if view_n > 0.0 {
        (
          (ax - self_vx * align_w) / view_n,
          (ay - self_vy * align_w) / view_n,
          cohx / view_n,
          cohy / view_n,
        )
      } else {
        (0.0, 0.0, 0.0, 0.0)
      };
      let (force_x, force_y) = (fx + sep_x + align_x + coh_x, fy + sep_y + align_y + coh_y);

      let (x, y) = (px, py);
      let (mut velx, mut vely) = (self_vx, self_vy);
      if cn > 0.0 {
        velx += cx / cn / dt;
        vely += cy / cn / dt;
      }
      if (velx * velx + vely * vely) < u.velocity_threshold as f64 {
        velx = 0.0;
        vely = 0.0;
      }
      let (freex, freey) = (velx + force_x * dt, vely + force_y * dt);
      let damping = 1.0 + dt * u.friction as f64 * (freex * freex + freey * freey).sqrt();
      let (mut vx, mut vy) = (freex / damping, freey / damping);
      let speed = (vx * vx + vy * vy).sqrt();
      if speed >= 1e-3 {
        let dv = (u.target_speed as f64 - speed) * u.drive_rate as f64 * dt;
        vx += (vx / speed) * dv;
        vy += (vy / speed) * dv;
      }
      let (mut nx, mut ny) = (x + vx * dt, y + vy * dt);
      for (n, size) in [(&mut nx, world.x as f64), (&mut ny, world.y as f64)] {
        if n.abs() > size / 2.0 {
          *n -= size * (*n / size).round();
        }
      }

      GpuParticle {
        pos: [nx as f32, ny as f32],
        vel: [vx as f32, vy as f32],
        species: me.species,
        id: me.id,
      }
    })
    .collect()
}

#[test]
fn one_step_matches_reference() {
  {
    let mut rng = StdRng::seed_from_u64(7);
    let spec = spec(&mut rng, 4);
    // Mało kandydatów w sąsiedztwie (poniżej progu próbkowania), ale dość gęsto na pary.
    let particles = random_particles(&mut rng, 700, 4);
    let expected = reference_step(&spec, &particles, WORLD);
    let Some(mut harness) = Harness::new(SimOptions::default(), spec, particles, WORLD) else {
      return;
    };
    assert_close(&harness.run(1), &expected, "jeden krok");
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
  let expected = reference_step(&spec, &particles, world);
  let Some(mut harness) = Harness::new(SimOptions::default(), spec, particles, world) else {
    return;
  };
  assert_close(&harness.run(1), &expected, "duży świat");
}

/// Krok zgodny z wyrocznią dla różnej liczby typów: shader czyta współczynniki sił inaczej dla
/// do 4 typów (`vec4`), do 8 (dwa `vec4`) i więcej (macierz z bufora).
#[test]
fn one_step_matches_reference_for_any_number_of_types() {
  for types in [1, 2, 4, 5, 8, 9, 13] {
    let mut rng = StdRng::seed_from_u64(40 + types as u64);
    let spec = spec(&mut rng, types);
    let particles = random_particles(&mut rng, 700, types);
    let expected = reference_step(&spec, &particles, WORLD);
    let Some(mut harness) = Harness::new(SimOptions::default(), spec, particles, WORLD) else {
      return;
    };
    assert_close(&harness.run(1), &expected, &format!("{types} typów"));
  }
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
/// kroku, bez rekompilacji i bez nowego `GpuSim`.
#[test]
fn spec_changes_at_runtime() {
  let mut rng = StdRng::seed_from_u64(31);
  let first = spec(&mut rng, 4);
  let particles = random_particles(&mut rng, 700, 4);
  let Some(mut harness) = Harness::new(
    SimOptions::default(),
    first.clone(),
    particles.clone(),
    WORLD,
  ) else {
    return;
  };
  assert_close(
    &harness.run(1),
    &reference_step(&first, &particles, WORLD),
    "start",
  );

  let mut second = spec(&mut rng, 4);
  second.params.friction = 0.2;
  second.params.repulsion_radius = 2.0;
  second.params.kernel_middle = 25.0;
  second.params.kernel_width = 8.0;
  harness.set_spec(second.clone());
  harness.set_state(particles.clone());
  let actual = harness.run(1);
  assert_close(
    &actual,
    &reference_step(&second, &particles, WORLD),
    "po zmianie spec",
  );
  assert_ne!(
    actual[0].pos,
    reference_step(&first, &particles, WORLD)[0].pos,
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
  spec.params.separation_weight = 0.0;
  spec.params.alignment_weight = 0.0;
  spec.params.coherence_weight = 0.0;
  spec.params.drive_rate = 0.0;
  spec
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

#[test]
fn separation_only_pushes_close_neighbours_apart() {
  let mut spec = isolated_spec(1);
  spec.params.separation_radius = 10.0;
  spec.params.separation_weight = 2.5;
  let particles = vec![
    particle(0, [0.0, 0.0], [0.0, 0.0]),
    particle(1, [4.0, 0.0], [0.0, 0.0]),
    // Poza separation_radius, ale w INTERACTION_RADIUS.
    particle(2, [60.0, 0.0], [0.0, 0.0]),
    particle(3, [60.0, 15.0], [0.0, 0.0]),
    // Bardzo blisko siebie (< 1e-3): ograniczenie max(dist, 1e-3) w nodes::separation.
    particle(4, [0.0, 100.0], [0.0, 0.0]),
    particle(5, [0.0002, 100.0], [0.0, 0.0]),
    // Dwóch sąsiadów w separation_radius: a_sep uśrednia wkłady przez sep_n.
    particle(6, [-100.0, 0.0], [0.0, 0.0]),
    particle(7, [-96.0, 0.0], [0.0, 0.0]),
    particle(8, [-100.0, 4.0], [0.0, 0.0]),
  ];
  let expected = reference_step(&spec, &particles, WORLD);
  let Some(mut harness) = Harness::new(SimOptions::default(), spec, particles, WORLD) else {
    return;
  };
  let actual = harness.run(1);
  assert_close(&actual, &expected, "separacja");
  assert!(actual[0].vel[0] < 0.0 && actual[1].vel[0] > 0.0);
  assert_eq!(actual[2].vel, [0.0, 0.0]);
  assert_eq!(actual[3].vel, [0.0, 0.0]);
  assert!((actual[4].vel[0] + 25.0).abs() < 1e-2);
  assert!((actual[6].vel[0] - 0.5 * actual[0].vel[0]).abs() < 1e-4);
  assert!((actual[6].vel[0] - actual[6].vel[1]).abs() < 1e-4);
}

#[test]
fn alignment_only_aligns_velocities_and_quantizes_neighbour_vel() {
  let mut spec = isolated_spec(1);
  spec.params.vision_radius = 25.0;
  spec.params.alignment_weight = 20.0;
  let particles = vec![
    // Cząstka 0 wyrównuje prędkość do średniej z (1, 2) dzięki align_w i view_n.
    particle(0, [0.0, 0.0], [10.0, 0.0]),
    particle(1, [10.0, 0.0], [10.0, 20.0]),
    particle(2, [0.0, 10.0], [10.0, -4.0]),
    // Para poza vision_radius = 25.0 (w INTERACTION_RADIUS = 40.0): brak wpływu.
    particle(3, [100.0, 0.0], [5.0, 0.0]),
    particle(4, [130.0, 0.0], [5.0, 50.0]),
    // Identyczne prędkości wejściowe: różnica wynika wyłącznie z kwantyzacji f16 prędkości sąsiada.
    particle(5, [-150.0, 0.0], [125.125, 123.456]),
    particle(6, [-145.0, 0.0], [125.125, 123.456]),
  ];
  let expected = reference_step(&spec, &particles, WORLD);
  let Some(mut harness) = Harness::new(SimOptions::default(), spec, particles, WORLD) else {
    return;
  };
  let actual = harness.run(1);
  assert_close(&actual, &expected, "wyrównanie");
  assert!(actual[0].vel[1] > 1.0);
  assert!((actual[0].vel[0] - 10.0).abs() < 1e-4);
  assert_eq!(actual[3].vel, [5.0, 0.0]);
  assert_eq!(actual[4].vel, [5.0, 50.0]);
  assert!(actual[5].vel[1] - 123.456 < -2e-3);
}

#[test]
fn alignment_uses_velocity_before_overlap_correction() {
  let mut spec = isolated_spec(1);
  spec.params.repulsion_radius = 1.0;
  spec.params.vision_radius = 25.0;
  spec.params.alignment_weight = 10.0;
  let particles = vec![
    particle(0, [0.0, 0.0], [50.0, 0.0]),
    particle(1, [0.5, 0.0], [0.0, 0.0]),
  ];
  let expected = reference_step(&spec, &particles, WORLD);
  let Some(mut harness) = Harness::new(SimOptions::default(), spec, particles, WORLD) else {
    return;
  };
  let actual = harness.run(1);
  assert_close(&actual, &expected, "wyrównanie przed rozsuwaniem");
}

#[test]
fn cohesion_only_pulls_toward_neighbours_within_vision_radius() {
  let mut spec = isolated_spec(1);
  spec.params.vision_radius = 25.0;
  spec.params.coherence_weight = 2.0;
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
  let expected = reference_step(&spec, &particles, WORLD);
  let Some(mut harness) = Harness::new(SimOptions::default(), spec, particles, WORLD) else {
    return;
  };
  let actual = harness.run(1);
  assert_close(&actual, &expected, "spójność");
  assert!(actual[0].vel[0] > 0.0 && actual[0].vel[1] > 0.0);
  assert_eq!(actual[3].vel, [0.0, 0.0]);
  assert_eq!(actual[4].vel, [0.0, 0.0]);
  assert!(actual[5].vel[0] < 0.0 && actual[6].vel[0] > 0.0);
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
  let expected = reference_step(&spec, &particles, WORLD);
  let Some(mut harness) = Harness::new(SimOptions::default(), spec, particles, WORLD) else {
    return;
  };
  let actual = harness.run(1);
  assert_close(&actual, &expected, "drive");

  let speed = |p: &GpuParticle| (p.vel[0] * p.vel[0] + p.vel[1] * p.vel[1]).sqrt();
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

#[test]
fn zero_boid_weights_match_reference_and_ignore_boid_radii() {
  let mut rng = StdRng::seed_from_u64(19);
  let default_spec = spec(&mut rng, 4);
  let mut zero_boids = default_spec.clone();
  zero_boids.params.separation_weight = 0.0;
  zero_boids.params.alignment_weight = 0.0;
  zero_boids.params.coherence_weight = 0.0;
  zero_boids.params.drive_rate = 0.0;

  let particles = random_particles(&mut rng, 700, 4);
  let expected = reference_step(&zero_boids, &particles, WORLD);
  let Some(mut harness) = Harness::new(
    SimOptions::default(),
    zero_boids.clone(),
    particles.clone(),
    WORLD,
  ) else {
    return;
  };
  let actual = harness.run(1);
  assert_close(&actual, &expected, "wyzerowane wagi boidów");

  let mut no_radii = zero_boids;
  no_radii.params.separation_radius = 0.0;
  no_radii.params.vision_radius = 0.0;
  no_radii.params.target_speed = 0.0;
  harness.set_spec(no_radii.clone());
  harness.set_state(particles.clone());
  let actual_no_radii = harness.run(1);
  assert_close(
    &actual_no_radii,
    &reference_step(&no_radii, &particles, WORLD),
    "wyzerowane wagi i promienie boidów",
  );
  assert_close(
    &actual_no_radii,
    &actual,
    "promienie boidów przy zerowych wagach nie zmieniają wyniku",
  );

  let with_boids = reference_step(&default_spec, &particles, WORLD);
  assert!(
    actual
      .iter()
      .zip(&with_boids)
      .any(|(a, b)| (a.vel[0] - b.vel[0]).abs() > 1e-3),
    "wyzerowane wagi boidów powinny różnić się od domyślnych"
  );
}

#[test]
fn velocity_above_f16_max_does_not_propagate_nan() {
  // Prędkości powyżej f16::MAX (65504) w pack_neighbour są ograniczane przed pack2x16float,
  // więc przeciwnie skierowane szybkie cząstki nie dają ±Inf ani NaN u sąsiadów.
  let mut isolated = isolated_spec(1);
  isolated.params.vision_radius = 40.0;
  isolated.params.alignment_weight = 1.0;
  let one_step = vec![
    particle(0, [0.0, 0.0], [100_000.0, -90_000.0]),
    particle(1, [-10.0, 0.0], [-100_000.0, 90_000.0]),
    particle(2, [5.0, 0.0], [0.0, 0.0]),
    particle(3, [-5.0, 0.0], [10.0, -5.0]),
  ];
  let expected = reference_step(&isolated, &one_step, WORLD);
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
  assert_close(&actual, &expected, "prędkość > 65504 w jednym kroku");
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
