//! Opcje i stan symulacji, przełączane w trakcie (klawisze w `ui`) i ustawiane flagami przy
//! starcie. Każda kombinacja opcji to osobny wariant pipeline'u sił na GPU (shader defs), więc
//! shader nie sprawdza opcji w pętli.

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

/// Wartości `Overlap` jako klucze wariantów pipeline'u sił.
pub const SPRING: u8 = Overlap::Spring as u8;
pub const RESOLVE: u8 = Overlap::Resolve as u8;
pub const ADAPTIVE_RANGE: u8 = Overlap::AdaptiveRange as u8;

#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SimOptions {
  /// Siła cząstki skalowana w dół, gdy ma więcej sąsiadów niż typowo (zadanie 024).
  pub average_forces: bool,
  pub overlap: Overlap,
}

/// Symulacja biegnie albo stoi (spacja w `ui`).
#[derive(States, Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum SimState {
  #[default]
  Running,
  Paused,
}

#[cfg(test)]
mod tests;
