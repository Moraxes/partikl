// Wspólny układ wiązań wszystkich przebiegów compute (grupa 0). Numery i nazwy muszą zgadzać
// się z `Binding` w `src/gpu/compute/bindings.rs` (sprawdza to test).

import super::types::{Particle, Neighbour, EngineUniforms, RuleParams};

@group(0) @binding(0) var<uniform> uniforms: EngineUniforms;
// Trwały stan cząstek (czytany też przez render). Po kroku leży w kolejności siatki (patrz
// `sorted_state`), więc render widzi cząstki spójnie przestrzennie; `Particle::id` jest
// stałym identyfikatorem, a nie indeksem.
@group(0) @binding(1) var<storage, read_write> state: array<Particle>;
// Sąsiedzi (`Neighbour`) w kolejności siatki, spakowani do `vec4` (jeden odczyt 16 B w
// pętli po sąsiadach); zapis i odczyt tylko przez `store_neighbour` / `load_neighbour`.
@group(0) @binding(2) var<storage, read_write> sorted_hot: array<vec4<f32>>;
// Pełne cząstki w kolejności siatki: scatter kopiuje je ze `state`, siły czytają stąd własną
// cząstkę i zapisują wynik do `state[j]`, więc stan sam układa się w kolejności siatki.
@group(0) @binding(3) var<storage, read_write> sorted_state: array<Particle>;
@group(0) @binding(4) var<storage, read_write> cell_counts: array<atomic<u32>>;
@group(0) @binding(5) var<storage, read_write> cell_offsets: array<u32>;
@group(0) @binding(6) var<storage, read_write> cell_cursors: array<atomic<u32>>;
// Sumy bloków sumy prefiksowej leżą w `cell_offsets` od indeksu `num_cells`.
// Macierz sił `matrix[a * T + b]`: siła, z jaką typ `a` działa na typ `b`.
@group(0) @binding(7) var<storage, read_write> force_matrix: array<f32>;
// Parametry reguł (promienie, jądro, tarcie…); wspólne dla wszystkich kroków klatki.
@group(0) @binding(8) var<uniform> rule_params: RuleParams;

// Układ `sorted_hot[slot]`: (pos.x, pos.y, bity species, crowding).
fn store_neighbour(slot: u32, p: Particle) {
  sorted_hot[slot] = vec4<f32>(p.pos, bitcast<f32>(p.species), p.crowding);
}

fn load_neighbour(slot: u32) -> Neighbour {
  let v = sorted_hot[slot];
  return Neighbour(v.xy, bitcast<u32>(v.z), v.w);
}
