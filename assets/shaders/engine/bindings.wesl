// Wspólny układ wiązań wszystkich przebiegów compute (grupa 0).

import super::types::{Particle, SimUniforms};

@group(0) @binding(0) var<uniform> uniforms: SimUniforms;
// Trwały stan cząstek (czytany też przez render).
@group(0) @binding(1) var<storage, read_write> state: array<Particle>;
// Kopia stanu w kolejności siatki.
@group(0) @binding(2) var<storage, read_write> sorted: array<Particle>;
@group(0) @binding(3) var<storage, read_write> cell_counts: array<atomic<u32>>;
@group(0) @binding(4) var<storage, read_write> cell_offsets: array<u32>;
@group(0) @binding(5) var<storage, read_write> cell_cursors: array<atomic<u32>>;
@group(0) @binding(6) var<storage, read_write> block_sums: array<u32>;
// Macierz sił `matrix[a * T + b]`: siła, z jaką typ `a` działa na typ `b`.
@group(0) @binding(7) var<storage, read_write> force_matrix: array<f32>;
