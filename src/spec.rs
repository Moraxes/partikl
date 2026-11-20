use bevy::prelude::*;

use serde::{Deserialize, Serialize};

/// Specyfikacja oddziaływań: macierz sił między typami cząstek.
#[derive(Default, Resource)]
pub struct ParticleSpec {
  pub interactions: Vec<Interaction>,
}

/// Wiersz macierzy sił dla jednego typu cząstki `a`: `force_coeffs[b]` to siła,
/// z jaką cząstka typu `a` działa na cząstkę typu `b`. Format pliku `.ron`
/// to lista takich wierszy.
#[derive(Default, Deserialize, Serialize, Debug)]
pub struct Interaction {
  pub force_coeffs: Vec<f32>,
}
