// Wiązania silnika grupy 0 wspólne dla przebiegów compute. Numery i nazwy muszą zgadzać się z
// `Binding` w `src/gpu/compute/bindings.rs` (sprawdza to test). Uniformy (0) są w `uniforms.wesl`,
// a dane reguł (7, 8) w `rules/bindings.wesl`.

import super::types::Particle;
import super::super::rules::model::{Model, Neighbour, pack_neighbour, unpack_neighbour};

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
@group(0) @binding(6) var<storage, read_write> cell_cursors: array<atomic<u32>>;
// Sumy bloków sumy prefiksowej leżą w `cell_offsets` od indeksu `num_cells`.
// Model cząstek (`Model` z reguł) równolegle do `state`, a `sorted_model` do `sorted_state`:
// silnik przenosi je razem z cząstką i nie zna ich pól.
@group(0) @binding(9) var<storage, read_write> model: array<Model>;
@group(0) @binding(10) var<storage, read_write> sorted_model: array<Model>;

fn store_neighbour(slot: u32, p: Particle, m: Model) {
  sorted_hot[slot] = pack_neighbour(p, m);
}

fn load_neighbour(slot: u32) -> Neighbour {
  return unpack_neighbour(sorted_hot[slot]);
}
