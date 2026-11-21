// Wspólny układ wiązań wszystkich przebiegów compute (grupa 0). Numery i nazwy muszą zgadzać
// się z `Binding` w `src/gpu/compute/bindings.rs` (sprawdza to test).

import super::types::{Particle, Neighbour, SimUniforms};

@group(0) @binding(0) var<uniform> uniforms: SimUniforms;
// Trwały stan cząstek (czytany też przez render).
@group(0) @binding(1) var<storage, read_write> state: array<Particle>;
// Sąsiedzi (`Neighbour`) w kolejności siatki, spakowani do `vec4` (jeden odczyt 16 B w
// pętli po sąsiadach); zapis i odczyt tylko przez `store_neighbour` / `load_neighbour`.
@group(0) @binding(2) var<storage, read_write> sorted_hot: array<vec4<f32>>;
// Indeks w `state` cząstki z danego miejsca w kolejności siatki.
@group(0) @binding(3) var<storage, read_write> sorted_ids: array<u32>;
@group(0) @binding(4) var<storage, read_write> cell_counts: array<atomic<u32>>;
@group(0) @binding(5) var<storage, read_write> cell_offsets: array<u32>;
@group(0) @binding(6) var<storage, read_write> cell_cursors: array<atomic<u32>>;
// Sumy bloków sumy prefiksowej leżą w `cell_offsets` od indeksu `num_cells`.
// Macierz sił `matrix[a * T + b]`: siła, z jaką typ `a` działa na typ `b`.
@group(0) @binding(7) var<storage, read_write> force_matrix: array<f32>;

// Układ `sorted_hot[slot]`: (pos.x, pos.y, bity species, crowding).
fn store_neighbour(slot: u32, p: Particle) {
  sorted_hot[slot] = vec4<f32>(p.pos, bitcast<f32>(p.species), p.crowding);
}

fn load_neighbour(slot: u32) -> Neighbour {
  let v = sorted_hot[slot];
  return Neighbour(v.xy, bitcast<u32>(v.z), v.w);
}
