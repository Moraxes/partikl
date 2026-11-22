//! Wiązania grupy 0 wspólne dla wszystkich przebiegów kroku (compute).
//!
//! Numer wiązania to numer wariantu [`Binding`], a nazwa to nazwa zmiennej w WESL: silnika
//! (`shaders/engine/{uniforms,bindings}.wesl`) albo reguł (`shaders/rules/bindings.wesl`); zgodność
//! z plikami WESL sprawdza test. Dodanie wiązania to:
//! wariant tutaj (w kolejności numerów), wpis w `layout_entry` i `create_group` (kompilator
//! pilnuje wyczerpującego `match`) i zmienna w odpowiednim pliku WESL.

use bevy::render::render_resource::binding_types::{storage_buffer_sized, uniform_buffer};
use bevy::render::render_resource::*;
use bevy::render::renderer::RenderDevice;

use super::{EngineUniforms, RuleParams};
use super::buffers::Buffers;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Binding {
  /// `uniforms`: stałe i geometria kroku; przesunięcie dynamiczne wybiera krok klatki.
  Uniforms = 0,
  /// `state`: trwały stan cząstek (wspólny z renderem).
  State = 1,
  /// `sorted_hot`: sąsiedzi w kolejności siatki, spakowani do `vec4`.
  SortedHot = 2,
  /// `sorted_state`: pełne cząstki w kolejności siatki (z nich siły zapisują `state`).
  SortedState = 3,
  /// `cell_counts`: liczba cząstek w komórce.
  CellCounts = 4,
  /// `cell_offsets`: początek komórki w kolejności siatki (i sumy bloków za komórkami).
  CellOffsets = 5,
  /// `next_counts`: histogram komórek następnego kroku, liczony w przebiegu sił z nowych pozycji.
  NextCounts = 6,
  /// `force_matrix`: macierz sił `[typ a * T + typ b]`.
  ForceMatrix = 7,
  /// `rule_params`: parametry reguł, wspólne dla kroków klatki (bez przesunięcia dynamicznego).
  RuleParams = 8,
  /// `model`: pola modelu cząstek z reguł, równolegle do `state` (wspólny z `GpuSim::model`).
  Model = 9,
  /// `sorted_model`: model w kolejności siatki, równolegle do `sorted_state`.
  SortedModel = 10,
  /// `cell_ranks`: ranga cząstki `state[i]` w jej komórce (z histogramu przebiegu sił lub `count`).
  CellRanks = 11,
}

impl Binding {
  pub const ALL: [Binding; 12] = [
    Binding::Uniforms,
    Binding::State,
    Binding::SortedHot,
    Binding::SortedState,
    Binding::CellCounts,
    Binding::CellOffsets,
    Binding::NextCounts,
    Binding::ForceMatrix,
    Binding::RuleParams,
    Binding::Model,
    Binding::SortedModel,
    Binding::CellRanks,
  ];

  /// Nazwa zmiennej w `bindings.wesl` (do testu zgodności z plikiem).
  #[cfg(test)]
  pub fn wesl_name(self) -> &'static str {
    match self {
      Binding::Uniforms => "uniforms",
      Binding::State => "state",
      Binding::SortedHot => "sorted_hot",
      Binding::SortedState => "sorted_state",
      Binding::CellCounts => "cell_counts",
      Binding::CellOffsets => "cell_offsets",
      Binding::NextCounts => "next_counts",
      Binding::ForceMatrix => "force_matrix",
      Binding::RuleParams => "rule_params",
      Binding::Model => "model",
      Binding::SortedModel => "sorted_model",
      Binding::CellRanks => "cell_ranks",
    }
  }

  /// `var<uniform>` w WESL; pozostałe to `var<storage, read_write>`.
  #[cfg(test)]
  pub fn is_uniform(self) -> bool {
    matches!(self, Binding::Uniforms | Binding::RuleParams)
  }

  fn layout_entry(self) -> BindGroupLayoutEntry {
    let entry = match self {
      Binding::Uniforms => uniform_buffer::<EngineUniforms>(true),
      Binding::RuleParams => uniform_buffer::<RuleParams>(false),
      _ => storage_buffer_sized(false, None),
    };
    entry.build(self as u32, ShaderStages::COMPUTE)
  }
}

pub fn layout() -> BindGroupLayoutDescriptor {
  let entries: Vec<_> = Binding::ALL.into_iter().map(Binding::layout_entry).collect();
  BindGroupLayoutDescriptor::new("partikl_sim", &entries)
}

/// Bind group kroku: `uniforms` to wiązanie bufora uniformów, `state` i `model` bufory stanu i
/// modelu cząstek.
pub fn create_group(
  device: &RenderDevice,
  layout: &BindGroupLayout,
  uniforms: BindingResource,
  state: &Buffer,
  model: &Buffer,
  buffers: &Buffers,
) -> BindGroup {
  let entries: Vec<_> = Binding::ALL
    .into_iter()
    .map(|binding| BindGroupEntry {
      binding: binding as u32,
      resource: match binding {
        Binding::Uniforms => uniforms.clone(),
        Binding::State => state.as_entire_binding(),
        Binding::SortedHot => buffers.sorted_hot.as_entire_binding(),
        Binding::SortedState => buffers.sorted_state.as_entire_binding(),
        Binding::CellCounts => buffers.cell_counts.as_entire_binding(),
        Binding::CellOffsets => buffers.cell_offsets.as_entire_binding(),
        Binding::NextCounts => buffers.next_counts.as_entire_binding(),
        Binding::ForceMatrix => buffers.force_matrix.as_entire_binding(),
        Binding::RuleParams => buffers.rule_params.as_entire_binding(),
        Binding::Model => model.as_entire_binding(),
        Binding::SortedModel => buffers.sorted_model.as_entire_binding(),
        Binding::CellRanks => buffers.cell_ranks.as_entire_binding(),
      },
    })
    .collect();
  device.create_bind_group("partikl_sim", layout, &entries)
}

#[cfg(test)]
mod tests;
