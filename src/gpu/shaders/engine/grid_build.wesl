// Budowa siatki. Histogram następnego kroku liczy przebieg sił (`next_counts`, `cell_ranks`), więc
// zwykły krok to: `clear` (przejęcie histogramu), suma prefiksowa, `scatter` bez atomics.
// `count` liczy histogram i rangi od zera: tylko w pierwszym kroku po wgraniu stanu.

import super::types::{Particle, cell_index};
import super::uniforms::uniforms;
import super::bindings::{state, sorted_state, cell_counts, cell_offsets, next_counts, cell_ranks, store_neighbour};
@if(HAS_MODEL)
import super::bindings::{model, sorted_model};

@compute @workgroup_size(64)
fn clear(@builtin(global_invocation_id) gid: vec3<u32>) {
  let c = gid.x;
  if c < uniforms.num_cells {
    atomicStore(&cell_counts[c], atomicLoad(&next_counts[c]));
    atomicStore(&next_counts[c], 0u);
  }
}

@compute @workgroup_size(64)
fn count(@builtin(global_invocation_id) gid: vec3<u32>) {
  let i = gid.x;
  if i >= uniforms.num_particles {
    return;
  }
  // `next_counts` jest wyzerowany przed krokiem; `clear` przenosi go do `cell_counts`.
  cell_ranks[i] = atomicAdd(&next_counts[cell_index(state[i].pos, uniforms)], 1u);
}

@compute @workgroup_size(64)
fn scatter(@builtin(global_invocation_id) gid: vec3<u32>) {
  let i = gid.x;
  if i >= uniforms.num_particles {
    return;
  }
  let p = state[i];
  let dst = cell_offsets[cell_index(p.pos, uniforms)] + cell_ranks[i];
  sorted_state[dst] = p;
  @if(HAS_MODEL) {
    let m = model[i];
    store_neighbour(dst, p, m);
    sorted_model[dst] = m;
  }
  @else {
    store_neighbour(dst, p);
  }
}

// Tryb deterministyczny: kolejność cząstek w komórce zależy od harmonogramu `atomicAdd` w
// histogramie (rangi), więc sortujemy każdą komórkę po id (jeden wątek na komórkę, sortowanie przez
// wstawianie, cząstki 24 B z opcjonalnym modelem) i odtwarzamy gorące dane. Wtedy sumy f32 mają
// stałą kolejność, a trajektorie są takie same co do bitu.
@compute @workgroup_size(64)
fn sort_cells(@builtin(global_invocation_id) gid: vec3<u32>) {
  let c = gid.x;
  if c >= uniforms.num_cells {
    return;
  }
  let start = cell_offsets[c];
  let n = atomicLoad(&cell_counts[c]);
  for (var i = 1u; i < n; i++) {
    let key = sorted_state[start + i];
    @if(HAS_MODEL)
    let key_model = sorted_model[start + i];
    var j = i;
    while j > 0u && sorted_state[start + j - 1u].id > key.id {
      sorted_state[start + j] = sorted_state[start + j - 1u];
      @if(HAS_MODEL)
      sorted_model[start + j] = sorted_model[start + j - 1u];
      j--;
    }
    sorted_state[start + j] = key;
    @if(HAS_MODEL)
    sorted_model[start + j] = key_model;
  }
  for (var s = start; s < start + n; s++) {
    @if(HAS_MODEL)
    store_neighbour(s, sorted_state[s], sorted_model[s]);
    @else
    store_neighbour(s, sorted_state[s]);
  }
}
