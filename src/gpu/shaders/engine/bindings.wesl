// Wiązania silnika grupy 0 wspólne dla przebiegów compute. Numery i nazwy muszą zgadzać się z
// `Binding` w `src/gpu/compute/bindings.rs` (sprawdza to test). Uniformy (0) są w `uniforms.wesl`,
// a dane reguł (7, 8) w `rules/bindings.wesl`.

import super::types::Particle;
import super::super::rules::model::{Neighbour, pack_neighbour, unpack_neighbour};
@if(HAS_MODEL)
import super::super::rules::model::Model;

// Trwały stan cząstek (czytany też przez render). Po kroku leży w kolejności siatki (patrz
// `sorted_state`), więc render widzi cząstki spójnie przestrzennie; `Particle::id` jest
// stałym identyfikatorem, a nie indeksem.
@group(0) @binding(1) var<storage, read_write> state: array<Particle>;
// Sąsiedzi (`Neighbour`) w kolejności siatki, spakowani przez reguły do `vec4` (jeden odczyt 16 B
// w pętli po sąsiadach); zapis i odczyt tylko przez `store_neighbour` / `load_neighbour`.
@group(0) @binding(2) var<storage, read_write> sorted_hot: array<vec4<f32>>;
// Pełne cząstki w kolejności siatki: scatter kopiuje je ze `state`, siły czytają stąd własną
// cząstkę i zapisują wynik do `state[j]`, więc stan sam układa się w kolejności siatki.
@group(0) @binding(3) var<storage, read_write> sorted_state: array<Particle>;
@group(0) @binding(4) var<storage, read_write> cell_counts: array<atomic<u32>>;
@group(0) @binding(5) var<storage, read_write> cell_offsets: array<u32>;
// Histogram komórek następnego kroku: przebieg sił dolicza do niego nową pozycję każdej cząstki,
// a `clear` następnego kroku przejmuje go do `cell_counts` i zeruje.
@group(0) @binding(6) var<storage, read_write> next_counts: array<atomic<u32>>;
// Sumy bloków sumy prefiksowej leżą w `cell_offsets` od indeksu `num_cells`.
// Model cząstek (`Model` z reguł) równolegle do `state`, a `sorted_model` do `sorted_state`:
// silnik przenosi je razem z cząstką i nie zna ich pól; bez pól reguł buforów nie ma w shaderze
// (bind group ma dummy 4 B dla wspólnego układu).
@if(HAS_MODEL)
@group(0) @binding(9) var<storage, read_write> model: array<Model>;
@if(HAS_MODEL)
@group(0) @binding(10) var<storage, read_write> sorted_model: array<Model>;
// Ranga `state[i]` w jej komórce (wynik `atomicAdd` na histogramie): miejsce w komórce przy scatterze.
@group(0) @binding(11) var<storage, read_write> cell_ranks: array<u32>;
// Wynik statystyk kroku (`stats.wesl`): licznik nie-skończonych, maksimum prędkości i sumy grup (bity f32).
@group(0) @binding(12) var<storage, read_write> stats: array<atomic<u32>>;

@if(HAS_MODEL)
fn store_neighbour(slot: u32, p: Particle, m: Model) {
  sorted_hot[slot] = pack_neighbour(p, m);
}
@else
fn store_neighbour(slot: u32, p: Particle) {
  sorted_hot[slot] = pack_neighbour(p);
}

fn load_neighbour(slot: u32) -> Neighbour {
  return unpack_neighbour(sorted_hot[slot]);
}
