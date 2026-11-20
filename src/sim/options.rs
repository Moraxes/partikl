//! Opcjonalne zachowania symulacji, przełączane w trakcie (klawisze w `ui`) i ustawiane flagami
//! przy starcie. Każda kombinacja to osobny wariant `compute_forces` z `run_if`, więc jądro sił
//! nie sprawdza opcji w pętli, a w schemacie i profilu widać, który wariant działa.

use std::fmt;

use bevy::prelude::*;
use clap::ValueEnum;

/// Jak cząstki reagują na nakładanie się (odległość poniżej `REPULSION_RADIUS`).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, ValueEnum)]
#[repr(u8)]
pub enum Overlap {
  /// Sprężyna odpychania o stałym zasięgu i sztywności.
  #[default]
  Spring,
  /// Rozsuwanie nakładających się par poprawką pozycji zamiast sprężyny.
  Resolve,
  /// Sprężyna o zasięgu rosnącym z gęstością wokół cząstek.
  AdaptiveRange,
}

impl Overlap {
  pub const ALL: [Overlap; 3] = [Overlap::Spring, Overlap::Resolve, Overlap::AdaptiveRange];

  /// Następny tryb (cyklicznie), dla klawisza przełączającego.
  pub fn next(self) -> Overlap {
    Overlap::ALL[(self as usize + 1) % Overlap::ALL.len()]
  }
}

impl fmt::Display for Overlap {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(match self {
      Overlap::Spring => "spring",
      Overlap::Resolve => "resolve",
      Overlap::AdaptiveRange => "adaptive range",
    })
  }
}

/// Wartości `Overlap` jako parametry stałe wariantów `compute_forces`.
pub const SPRING: u8 = Overlap::Spring as u8;
pub const RESOLVE: u8 = Overlap::Resolve as u8;
pub const ADAPTIVE_RANGE: u8 = Overlap::AdaptiveRange as u8;

#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SimOptions {
  /// Siła cząstki skalowana w dół, gdy ma więcej sąsiadów niż typowo (zadanie 024).
  pub average_forces: bool,
  pub overlap: Overlap,
}

impl SimOptions {
  /// Czy wariant `compute_forces::<AVERAGE, OVERLAP>` ma działać przy tych opcjach.
  pub fn selects(&self, average: bool, overlap: u8) -> bool {
    self.average_forces == average && self.overlap as u8 == overlap
  }
}
