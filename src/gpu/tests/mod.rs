//! Testy GPU: jeden krok względem skalarnej wyroczni f64 (wszystkie warianty `SimOptions`)
//! oraz niezmienniki wielu kroków. Działają na prawdziwym adapterze (headless, bez okna);
//! bez GPU test jest pomijany z komunikatem.
//!
//! Podział: `engine` (siatka, torus, próbkowanie, wsparcie harnessu; reguły diagnostyczne i wyrocznia
//! brute force), `default_rules` (wyrocznia f64 dla fizyki `rules/default_rules.wesl`), `render`
//! (rysowanie do obrazu), `rules` (źródło modułu reguł: plik, przeładowanie). Wspólne pomocnicze są tutaj.

use bevy::render::render_resource::ShaderType;
use super::compute::{EngineUniforms, RuleParams};
use super::headless::Harness;
use super::*;
use crate::options::SimOptions;
use crate::spec::Interaction;

mod default_rules;
mod engine;
mod render;
mod rules;

/// Opcje wbudowanych reguł tak, jak widzi je wyrocznia f64 (`default_rules.rs`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Mode {
  average_forces: bool,
  overlap: Overlap,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum Overlap {
  #[default]
  Spring,
  Resolve,
  AdaptiveRange,
}

impl Mode {
  /// Te same opcje w postaci wyborów z deklaracji reguł (`//@option` w `default_rules.wesl`).
  fn sim(self) -> SimOptions {
    let overlap = match self.overlap {
      Overlap::Spring => "spring",
      Overlap::Resolve => "resolve",
      Overlap::AdaptiveRange => "adaptive_range",
    };
    SimOptions::default().with("average_forces", if self.average_forces { "on" } else { "off" }).with("overlap", overlap)
  }
}

const WORLD: Vec2 = Vec2::new(500.0, 330.0);

fn spec(rng: &mut StdRng, types: usize) -> ParticleSpec {
  ParticleSpec::new(
    (0..types)
      .map(|_| Interaction { force_coeffs: (0..types).map(|_| rng.random_range(-500.0..500.0)).collect() })
      .collect(),
  )
}

fn random_particles(rng: &mut StdRng, n: usize, types: usize) -> Vec<GpuParticle> {
  initial_particles(n, types, WORLD, rng)
}

/// Stan cząstek razem z modelem wbudowanych reguł (`crowding` na cząstkę, indeksowane jak cząstki
/// po id); wyrocznia f64 i `assert_close` porównują oba.
#[derive(Clone, Debug)]
struct State {
  particles: Vec<GpuParticle>,
  crowding: Vec<f32>,
}

impl State {
  /// Z zerowym `crowding`, jak na starcie aplikacji.
  fn new(particles: Vec<GpuParticle>) -> State {
    let crowding = vec![0.0; particles.len()];
    State { particles, crowding }
  }

  fn with_random_crowding(mut self, rng: &mut StdRng) -> State {
    self.crowding.iter_mut().for_each(|c| *c = rng.random_range(0.0..120.0));
    self
  }
}

/// Harness z wbudowanymi regułami i zadanym stanem startowym (z modelem).
fn harness_for(options: SimOptions, spec: ParticleSpec, state: &State, world: Vec2) -> Option<Harness> {
  let mut harness = Harness::new(options, spec, state.particles.clone(), world)?;
  if state.crowding.iter().any(|&c| c != 0.0) {
    harness.set_model(state.crowding.clone());
  }
  Some(harness)
}

/// Wraca do stanu `state` (cząstki i model), bez kroku symulacji.
fn reset(harness: &mut Harness, state: &State) {
  harness.set_state(state.particles.clone());
  harness.set_model(state.crowding.clone());
}

/// Wykonuje kroki i zwraca stan z modelem.
fn run_state(harness: &mut Harness, steps: u64) -> State {
  let (particles, crowding) = harness.run_with_model::<f32>(steps);
  State { particles, crowding }
}

fn assert_close(actual: &State, expected: &State, context: &str) {
  assert_eq!(actual.particles.len(), expected.particles.len());
  for (i, (a, e)) in actual.particles.iter().zip(&expected.particles).enumerate() {
    assert_eq!(a.id, i as u32, "{context}");
    assert_eq!(a.species, e.species);
    for k in 0..2 {
      assert!((a.pos[k] - e.pos[k]).abs() < 1e-3, "{context} cząstka {i}: pos {:?} vs {:?}", a.pos, e.pos);
      assert!((a.last[k] - e.last[k]).abs() < 1e-3, "{context} cząstka {i}: last {:?} vs {:?}", a.last, e.last);
    }
    let (ac, ec) = (actual.crowding[i], expected.crowding[i]);
    assert!((ac - ec).abs() < 1e-3, "{context} cząstka {i}: crowding {ac} vs {ec}");
  }
}

/// Macierz sił wyzerowana (cząstki nie przyspieszają).
fn zero_forces(mut spec: ParticleSpec) -> ParticleSpec {
  for row in &mut spec.interactions {
    row.force_coeffs.iter_mut().for_each(|c| *c = 0.0);
  }
  spec
}

/// Niezmienniki stanu dla dowolnych reguł: wartości skończone, cząstki w świecie (z marginesem na
/// jedno zawinięcie), każdy identyfikator `0..n` dokładnie raz (nic nie zginęło ani się nie powtórzyło).
fn assert_invariants(state: &[GpuParticle], world: Vec2, context: &str) {
  let mut seen = vec![false; state.len()];
  for p in state {
    assert!(p.pos.iter().chain(&p.last).all(|v| v.is_finite()), "{context}: NaN/Inf w {p:?}");
    assert!(p.pos[0].abs() <= world.x / 2.0 + 1.0 && p.pos[1].abs() <= world.y / 2.0 + 1.0, "{context}: poza światem: {p:?}");
    assert!(!seen[p.id as usize], "{context}: powtórzone id {}", p.id);
    seen[p.id as usize] = true;
  }
  assert!(seen.iter().all(|&s| s), "{context}: zgubione cząstki");
}
