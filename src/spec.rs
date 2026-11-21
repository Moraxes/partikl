use bevy::prelude::*;
use bevy::render::RenderApp;
use bevy::render::extract_resource::ExtractResource;

use serde::{Deserialize, Serialize};

use crate::gpu::RuleParams;

/// Specyfikacja symulacji: macierz sił między typami cząstek i parametry reguł. Zmiana zasobu w
/// trakcie działania trafia do buforów GPU bez rekompilacji shaderów (liczba typów jest stała).
#[derive(Default, Clone, Debug, Resource, ExtractResource, Deserialize, Serialize)]
#[extract_app(RenderApp)]
pub struct ParticleSpec {
  pub interactions: Vec<Interaction>,
  /// Parametry reguł; brakujące pola mają wartości z `config.rs`.
  #[serde(default)]
  pub params: RuleParams,
}

impl ParticleSpec {
  /// Macierz sił z domyślnymi parametrami reguł.
  pub fn new(interactions: Vec<Interaction>) -> Self {
    ParticleSpec { interactions, params: RuleParams::default() }
  }

  /// Macierz sił spłaszczona jako `m[a * T + b]` (siła typu `a` na typ `b`).
  pub fn flat_matrix(&self) -> Vec<f32> {
    self.interactions.iter().flat_map(|row| row.force_coeffs.iter().copied()).collect()
  }
}

/// Wiersz macierzy sił dla jednego typu cząstki `a`: `force_coeffs[b]` to siła,
/// z jaką cząstka typu `a` działa na cząstkę typu `b`.
#[derive(Default, Clone, Deserialize, Serialize, Debug)]
pub struct Interaction {
  pub force_coeffs: Vec<f32>,
}
