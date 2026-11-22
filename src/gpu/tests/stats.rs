//! Statystyki stanu po kroku (`GpuStats`): liczba NaN, prędkość i energia względem liczenia na CPU
//! z tego samego stanu, oraz ścieżka wyłączona (bez wyniku).

use super::*;

/// `(nan, maks. prędkość, energia)` liczone na CPU z tego samego stanu, tak jak przebieg
/// `reduce_stats`: różnica pozycji zawinięta o świat, cząstki niefinite pomijane w max i energii.
fn cpu_stats(state: &[GpuParticle]) -> (u32, f32, f32) {
  let mut nan = 0;
  let mut max_speed = 0f32;
  let mut energy = 0f64;
  for p in state {
    if !p.pos.iter().chain(&p.vel).all(|v| v.is_finite()) {
      nan += 1;
      continue;
    }
    let v = Vec2::from(p.vel);
    max_speed = max_speed.max(v.length());
    energy += 0.5 * v.length_squared() as f64;
  }
  (nan, max_speed, energy as f32)
}

fn close(actual: f32, expected: f32) -> bool {
  (actual - expected).abs() <= 1e-3 * expected.abs().max(1e-6)
}

/// Normalny stan po kilku krokach: zero NaN, prędkość i energia zgodne z CPU.
#[test]
fn stats_match_cpu_on_normal_state() {
  let mut rng = StdRng::seed_from_u64(61);
  let spec = spec(&mut rng, 3);
  let particles = random_particles(&mut rng, 5_000, 3);
  let Some(mut harness) = Harness::new(SimOptions::default(), spec, particles, WORLD) else { return };
  harness.set_stats(true);
  // Pierwszy przebieg kompiluje pipeline, więc jego wynik może się nie pojawić.
  harness.step(1);
  harness.take_stats();
  harness.step(3);
  let stats = harness.take_stats().expect("statystyki po kroku");
  // Stan po ostatnim kroku (bez dalszych kroków).
  let state = harness.run(0);
  let (nan, max_speed, energy) = cpu_stats(&state);
  assert_eq!(stats.particles, 5_000);
  assert_eq!(stats.nan_count, 0);
  assert_eq!(nan, 0);
  assert!(close(stats.max_speed, max_speed), "maks. prędkość: GPU {} CPU {max_speed}", stats.max_speed);
  assert!(close(stats.kinetic_energy, energy), "energia: GPU {} CPU {energy}", stats.kinetic_energy);
}

/// Wstrzyknięty NaN w `pos` jednej cząstki: dokładnie jedna cząstka w liczniku.
#[test]
fn injected_nan_is_counted() {
  let mut rng = StdRng::seed_from_u64(67);
  let spec = spec(&mut rng, 3);
  let particles = random_particles(&mut rng, 5_000, 3);
  let mut broken = particles.clone();
  broken[0].pos[0] = f32::NAN;
  let Some(mut harness) = Harness::new(SimOptions::default(), spec, particles, WORLD) else { return };
  harness.set_stats(true);
  harness.step(1);
  harness.take_stats();
  harness.set_state(broken);
  // Sprawdzamy po jednym kroku: NaN może później psuć stan dalej.
  harness.step(1);
  let stats = harness.take_stats().expect("statystyki po kroku z NaN");
  assert_eq!(stats.nan_count, 1);
  assert!(stats.max_speed.is_finite(), "maks. prędkość: {}", stats.max_speed);
  assert!(stats.kinetic_energy.is_finite(), "energia: {}", stats.kinetic_energy);
}

/// Przy wyłączonych statystykach (domyślnie) nie ma wyniku.
#[test]
fn disabled_stats_give_nothing() {
  let mut rng = StdRng::seed_from_u64(71);
  let spec = spec(&mut rng, 3);
  let particles = random_particles(&mut rng, 2_000, 3);
  let Some(mut harness) = Harness::new(SimOptions::default(), spec, particles, WORLD) else { return };
  harness.step(3);
  assert_eq!(harness.take_stats(), None);
}
