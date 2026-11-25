//! Budżet kroków na klatkę: gdy krok GPU nie mieści się w stałym kroku czasu, symulacja zwalnia
//! zamiast wydłużać klatki.
//!
//! Stały krok (`DELTA_TIME`, 100 kroków na sekundę) goni zaległość: każda klatka liczy tyle
//! kroków, ile minęło taktów. Gdy krok kosztuje więcej niż takt (przy ok. 1M cząstek ok. 11 ms
//! wobec 10 ms), klatka z N krokami trwa N·11 ms, więc w następnej jest ich jeszcze więcej, aż
//! do `MAX_STEPS_PER_FRAME` (klatki po ok. 170 ms). Tutaj liczba kroków w klatce ma limit
//! (`StepBudget::limit`), a takty ponad limit przepadają: symulacja idzie wolniej niż w czasie
//! rzeczywistym (wskaźnik `SIM_SPEED` w HUD), a obraz zostaje płynny.
//!
//! Limit steruje regulator AIMD po czasie klatki: klatka dłuższa niż `TARGET_FRAME` z więcej niż
//! jednym krokiem zmniejsza limit (×0,7), a krótka klatka, w której takty przepadały, zwiększa
//! go o 1. Czas klatki jest miarą obciążenia GPU: render działa potokowo, więc główna pętla czeka
//! na wolniejszy z końców potoku.

use bevy::diagnostic::{Diagnostic, DiagnosticPath, Diagnostics, RegisterDiagnostic};
use bevy::prelude::*;

use super::{GpuParams, MAX_STEPS_PER_FRAME};

/// Tak długa klatka (50 fps) jest jeszcze płynna; dłuższa zmniejsza limit kroków.
pub const TARGET_FRAME: f32 = 0.020;
/// Ułamek czasu rzeczywistego, w jakim idzie symulacja (1,0 = czas rzeczywisty).
pub const SIM_SPEED: DiagnosticPath = DiagnosticPath::const_new("partikl/sim_speed");
/// Waga bieżącej klatki w średniej kroków (wygładzanie wskaźnika prędkości).
const SMOOTHING: f32 = 0.05;

#[derive(Resource, Debug)]
pub struct StepBudget {
  /// Najwięcej kroków policzonych w jednej klatce; reszta zaległych taktów przepada.
  pub limit: u32,
  /// Kroki policzone w tej klatce i takty stałego kroku (policzone razem z przepadłymi).
  taken: u32,
  wanted: u32,
  /// Średnie kroków i taktów na klatkę (do prędkości symulacji).
  taken_avg: f32,
  wanted_avg: f32,
}

impl Default for StepBudget {
  fn default() -> Self {
    StepBudget {
      limit: MAX_STEPS_PER_FRAME,
      taken: 0,
      wanted: 0,
      taken_avg: 0.0,
      wanted_avg: 0.0,
    }
  }
}

impl StepBudget {
  /// Czy w tej klatce jakiś takt przepadł (GPU jest wtedy za zegarem stałego kroku).
  pub fn dropped(&self) -> bool {
    self.wanted > self.taken
  }

  /// Prędkość symulacji względem czasu rzeczywistego; `None`, dopóki nie było taktów (pauza).
  pub fn speed(&self) -> Option<f32> {
    (self.wanted_avg > 1.0e-3).then(|| (self.taken_avg / self.wanted_avg).min(1.0))
  }
}

/// Następny limit kroków po klatce trwającej `frame` sekund, w której policzono `taken` kroków,
/// a `dropped` taktów przepadło.
pub fn next_limit(limit: u32, frame: f32, taken: u32, dropped: bool) -> u32 {
  if frame > TARGET_FRAME && taken > 1 {
    (limit * 7 / 10).max(1)
  } else if dropped && frame < 0.75 * TARGET_FRAME {
    (limit + 1).min(MAX_STEPS_PER_FRAME)
  } else {
    limit
  }
}

/// Takt stałego kroku: liczy krok, jeśli mieści się w limicie klatki, inaczej takt przepada.
pub fn count_step(mut params: ResMut<GpuParams>, mut budget: ResMut<StepBudget>) {
  budget.wanted += 1;
  if budget.taken < budget.limit {
    budget.taken += 1;
    params.step_count += 1;
  }
}

/// Koniec klatki: regulator limitu i wskaźnik prędkości symulacji.
pub fn end_frame(
  time: Res<Time<Real>>,
  mut budget: ResMut<StepBudget>,
  mut diagnostics: Diagnostics,
) {
  let dropped = budget.wanted > budget.taken;
  budget.limit = next_limit(budget.limit, time.delta_secs(), budget.taken, dropped);
  // Pauza (brak taktów) nie zmienia średnich, więc wskaźnik zostaje ostatnią wartością.
  if budget.wanted > 0 {
    budget.taken_avg += SMOOTHING * (budget.taken as f32 - budget.taken_avg);
    budget.wanted_avg += SMOOTHING * (budget.wanted as f32 - budget.wanted_avg);
  }
  if let Some(speed) = budget.speed() {
    diagnostics.add_measurement(&SIM_SPEED, || speed as f64 * 100.0);
  }
  budget.taken = 0;
  budget.wanted = 0;
}

pub struct StepBudgetPlugin;

impl Plugin for StepBudgetPlugin {
  fn build(&self, app: &mut App) {
    app
      .init_resource::<StepBudget>()
      .register_diagnostic(Diagnostic::new(SIM_SPEED).with_suffix("%"))
      .add_systems(Last, end_frame);
  }
}

#[cfg(test)]
mod tests;
