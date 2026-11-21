// Wspólny układ wiązań wszystkich przebiegów compute (grupa 0).

import super::types::{Particle, SimUniforms};

@group(0) @binding(0) var<uniform> uniforms: SimUniforms;
// Trwały stan cząstek (czytany też przez render).
@group(0) @binding(1) var<storage, read_write> state: array<Particle>;
// Gorące dane sąsiadów w kolejności siatki (czytane w pętli po sąsiadach):
// (x, y, typ jako bity u32, crowding). Reszta stanu zostaje w `state`.
@group(0) @binding(2) var<storage, read_write> sorted_hot: array<vec4<f32>>;
// Indeks w `state` cząstki z danego miejsca w kolejności siatki.
@group(0) @binding(3) var<storage, read_write> sorted_ids: array<u32>;
@group(0) @binding(4) var<storage, read_write> cell_counts: array<atomic<u32>>;
@group(0) @binding(5) var<storage, read_write> cell_offsets: array<u32>;
@group(0) @binding(6) var<storage, read_write> cell_cursors: array<atomic<u32>>;
// Sumy bloków sumy prefiksowej leżą w `cell_offsets` od indeksu `num_cells`.
// Macierz sił `matrix[a * T + b]`: siła, z jaką typ `a` działa na typ `b`.
@group(0) @binding(7) var<storage, read_write> force_matrix: array<f32>;
