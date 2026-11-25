//! Testy GPU: jeden krok względem skalarnej wyroczni f64 (wszystkie warianty `SimOptions`)
//! oraz niezmienniki wielu kroków. Działają na prawdziwym adapterze (headless, bez okna);
//! bez GPU test jest pomijany z komunikatem.
//!
//! Podział: `engine` (siatka, torus, próbkowanie, wsparcie harnessu; reguły diagnostyczne i wyrocznia
//! brute force), `default_rules` (wyrocznia f64 dla fizyki `rules/default_rules.wesl`), `render`
//! (rysowanie do obrazu), `rules` (źródło modułu reguł: plik, przeładowanie). Wspólne pomocnicze są tutaj.

use super::compute::{EngineUniforms, RuleParams};
use super::headless::Harness;
use super::*;
use crate::options::SimOptions;
use crate::spec::Interaction;
use bevy::render::render_resource::ShaderType;

mod default_rules;
mod engine;
mod force_scales;
mod render;
mod rules;
mod shaders;
mod stats;

const WORLD: Vec2 = Vec2::new(500.0, 330.0);

fn spec(rng: &mut StdRng, types: usize) -> ParticleSpec {
  ParticleSpec::new(
    (0..types)
      .map(|_| Interaction {
        force_coeffs: (0..types)
          .map(|_| rng.random_range(-500.0..500.0))
          .collect(),
      })
      .collect(),
  )
}

fn random_particles(rng: &mut StdRng, n: usize, types: usize) -> Vec<GpuParticle> {
  initial_particles(n, types, WORLD, rng)
}

fn assert_close(actual: &[GpuParticle], expected: &[GpuParticle], context: &str) {
  assert_eq!(actual.len(), expected.len());
  for (i, (a, e)) in actual.iter().zip(expected).enumerate() {
    assert_eq!(a.id, i as u32, "{context}");
    assert_eq!(a.species, e.species);
    for k in 0..2 {
      assert!(
        (a.pos[k] - e.pos[k]).abs() < 1e-3,
        "{context} cząstka {i}: pos {:?} vs {:?}",
        a.pos,
        e.pos
      );
      assert!(
        (a.vel[k] - e.vel[k]).abs() < 1e-3,
        "{context} cząstka {i}: vel {:?} vs {:?}",
        a.vel,
        e.vel
      );
    }
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
    assert!(
      p.pos.iter().chain(&p.vel).all(|v| v.is_finite()),
      "{context}: NaN/Inf w {p:?}"
    );
    assert!(
      p.pos[0].abs() <= world.x / 2.0 + 1.0 && p.pos[1].abs() <= world.y / 2.0 + 1.0,
      "{context}: poza światem: {p:?}"
    );
    assert!(!seen[p.id as usize], "{context}: powtórzone id {}", p.id);
    seen[p.id as usize] = true;
  }
  assert!(seen.iter().all(|&s| s), "{context}: zgubione cząstki");
}
