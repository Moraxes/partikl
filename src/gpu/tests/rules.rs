use super::*;
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn temp_rules(name: &str, source: &str) -> PathBuf {
  let path = std::env::temp_dir().join(format!("partikl-rules-{}-{name}.wesl", std::process::id()));
  std::fs::write(&path, source).unwrap();
  path
}

fn setup() -> (ParticleSpec, Vec<GpuParticle>) {
  let mut rng = StdRng::seed_from_u64(90);
  (spec(&mut rng, 3), random_particles(&mut rng, 800, 3, false))
}

/// Moduł reguł z pliku to ten sam kod co wbudowany: wynik bit w bit taki sam (tryb deterministyczny).
#[test]
fn rules_from_a_file_match_the_builtin_ones() {
  let (spec, particles) = setup();
  let options = SimOptions::default();
  let Some(mut builtin) = Harness::new(options, spec.clone(), particles.clone(), WORLD) else { return };
  builtin.set_deterministic(true);
  let expected = builtin.run(3);
  // Harness trzyma blokadę GPU: drugi można utworzyć dopiero po zwolnieniu pierwszego.
  drop(builtin);

  let path = temp_rules("copy", include_str!("../shaders/rules/default_rules.wesl"));
  let mut from_file = Harness::new(options, spec, particles, WORLD).unwrap();
  from_file.set_deterministic(true);
  from_file.set_rules(RuleSource::File(path));
  assert_eq!(from_file.run(3), expected);
}

/// Zmiana pliku reguł w trakcie działania przeładowuje moduł: diagnostyczne nic nie poruszają,
/// wbudowane tak.
#[test]
fn saving_the_rules_file_reloads_the_module() {
  let (spec, particles) = setup();
  let path = temp_rules("watched", include_str!("../shaders/rules/diagnostic_rules.wesl"));
  let Some(mut harness) = Harness::new(SimOptions::default(), spec, particles.clone(), WORLD) else { return };
  harness.set_rules(RuleSource::File(path.clone()));
  let still = harness.run(1);
  assert!(still.iter().zip(&particles).all(|(a, b)| a.pos == b.pos), "reguły diagnostyczne nie poruszają cząstek");

  std::fs::write(&path, include_str!("../shaders/rules/default_rules.wesl")).unwrap();
  // Plik jest sprawdzany co pół sekundy.
  let deadline = Instant::now() + Duration::from_secs(30);
  loop {
    assert!(Instant::now() < deadline, "moduł nie przeładował się");
    std::thread::sleep(Duration::from_millis(100));
    harness.update();
    if harness.run(1).iter().zip(&particles).any(|(a, b)| a.pos != b.pos) {
      break;
    }
  }
}

/// Plik, którego nie da się wczytać, zostawia poprzedni moduł (błąd tylko w logu).
#[test]
fn unreadable_rules_file_keeps_the_previous_module() {
  let (spec, particles) = setup();
  let Some(mut harness) = Harness::new(SimOptions::default(), spec, particles, WORLD) else { return };
  let before = harness.run(1);
  harness.set_rules(RuleSource::File(std::env::temp_dir().join("partikl-rules-does-not-exist.wesl")));
  harness.update();
  assert_eq!(harness.run(0), before);
}

#[test]
fn missing_rules_file_is_a_load_error() {
  let error = RuleSource::File(std::env::temp_dir().join("partikl-rules-nope.wesl")).read().unwrap_err();
  assert!(error.to_string().contains("partikl-rules-nope"));
}
