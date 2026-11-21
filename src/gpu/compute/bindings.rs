//! Wiązania grupy 0 wspólne dla wszystkich przebiegów kroku (compute).
//!
//! Numer wiązania to numer wariantu [`Binding`], a nazwa to nazwa zmiennej w
//! `shaders/engine/bindings.wesl`; zgodność z plikiem WESL sprawdza test. Dodanie wiązania to:
//! wariant tutaj (w kolejności numerów), wpis w `layout_entry` i `create_group` (kompilator
//! pilnuje wyczerpującego `match`) i zmienna w `bindings.wesl`.

use bevy::render::render_resource::binding_types::{storage_buffer_sized, uniform_buffer};
use bevy::render::render_resource::*;
use bevy::render::renderer::RenderDevice;

use super::SimUniforms;
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
  /// `sorted_ids`: indeks w `state` cząstki z danego miejsca w kolejności siatki.
  SortedIds = 3,
  /// `cell_counts`: liczba cząstek w komórce.
  CellCounts = 4,
  /// `cell_offsets`: początek komórki w kolejności siatki (i sumy bloków za komórkami).
  CellOffsets = 5,
  /// `cell_cursors`: kursory zapisu w scatterze.
  CellCursors = 6,
  /// `force_matrix`: macierz sił `[typ a * T + typ b]`.
  ForceMatrix = 7,
}

impl Binding {
  pub const ALL: [Binding; 8] = [
    Binding::Uniforms,
    Binding::State,
    Binding::SortedHot,
    Binding::SortedIds,
    Binding::CellCounts,
    Binding::CellOffsets,
    Binding::CellCursors,
    Binding::ForceMatrix,
  ];

  /// Nazwa zmiennej w `bindings.wesl`.
  pub fn wesl_name(self) -> &'static str {
    match self {
      Binding::Uniforms => "uniforms",
      Binding::State => "state",
      Binding::SortedHot => "sorted_hot",
      Binding::SortedIds => "sorted_ids",
      Binding::CellCounts => "cell_counts",
      Binding::CellOffsets => "cell_offsets",
      Binding::CellCursors => "cell_cursors",
      Binding::ForceMatrix => "force_matrix",
    }
  }

  /// `var<uniform>` w WESL; pozostałe to `var<storage, read_write>`.
  pub fn is_uniform(self) -> bool {
    self == Binding::Uniforms
  }

  fn layout_entry(self) -> BindGroupLayoutEntry {
    let entry = if self.is_uniform() { uniform_buffer::<SimUniforms>(true) } else { storage_buffer_sized(false, None) };
    entry.build(self as u32, ShaderStages::COMPUTE)
  }
}

pub fn layout() -> BindGroupLayoutDescriptor {
  let entries: Vec<_> = Binding::ALL.into_iter().map(Binding::layout_entry).collect();
  BindGroupLayoutDescriptor::new("partikl_sim", &entries)
}

/// Bind group kroku: `uniforms` to wiązanie bufora uniformów, `state` bufor stanu cząstek.
pub fn create_group(
  device: &RenderDevice,
  layout: &BindGroupLayout,
  uniforms: BindingResource,
  state: &Buffer,
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
        Binding::SortedIds => buffers.sorted_ids.as_entire_binding(),
        Binding::CellCounts => buffers.cell_counts.as_entire_binding(),
        Binding::CellOffsets => buffers.cell_offsets.as_entire_binding(),
        Binding::CellCursors => buffers.cell_cursors.as_entire_binding(),
        Binding::ForceMatrix => buffers.force_matrix.as_entire_binding(),
      },
    })
    .collect();
  device.create_bind_group("partikl_sim", layout, &entries)
}

#[cfg(test)]
mod tests;
