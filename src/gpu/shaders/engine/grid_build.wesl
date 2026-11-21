// Budowa siatki: zerowanie, histogram (atomicAdd), scatter w kolejności komórek.

import super::types::{Particle, cell_index};
import super::bindings::{uniforms, state, sorted_hot, sorted_ids, cell_counts, cell_offsets, cell_cursors};

@compute @workgroup_size(64)
fn clear(@builtin(global_invocation_id) gid: vec3<u32>) {
  if gid.x < uniforms.num_cells {
    atomicStore(&cell_counts[gid.x], 0u);
  }
}

@compute @workgroup_size(64)
fn count(@builtin(global_invocation_id) gid: vec3<u32>) {
  let i = gid.x;
  if i >= uniforms.num_particles {
    return;
  }
  atomicAdd(&cell_counts[cell_index(state[i].pos, uniforms)], 1u);
}

@compute @workgroup_size(64)
fn scatter(@builtin(global_invocation_id) gid: vec3<u32>) {
  let i = gid.x;
  if i >= uniforms.num_particles {
    return;
  }
  let p = state[i];
  let dst = atomicAdd(&cell_cursors[cell_index(p.pos, uniforms)], 1u);
  sorted_hot[dst] = vec4<f32>(p.pos, bitcast<f32>(p.species), p.crowding);
  sorted_ids[dst] = i;
}

// Tryb deterministyczny: kolejność cząstek w komórce zależy od harmonogramu `atomicAdd` w
// `scatter`, więc sortujemy każdą komórkę po id (jeden wątek na komórkę, sortowanie przez
// wstawianie) i odtwarzamy gorące dane. Wtedy sumy f32 mają stałą kolejność, a trajektorie
// są takie same co do bitu.
@compute @workgroup_size(64)
fn sort_cells(@builtin(global_invocation_id) gid: vec3<u32>) {
  let c = gid.x;
  if c >= uniforms.num_cells {
    return;
  }
  let start = cell_offsets[c];
  let n = atomicLoad(&cell_counts[c]);
  for (var i = 1u; i < n; i++) {
    let key = sorted_ids[start + i];
    var j = i;
    while j > 0u && sorted_ids[start + j - 1u] > key {
      sorted_ids[start + j] = sorted_ids[start + j - 1u];
      j--;
    }
    sorted_ids[start + j] = key;
  }
  for (var s = start; s < start + n; s++) {
    let p = state[sorted_ids[s]];
    sorted_hot[s] = vec4<f32>(p.pos, bitcast<f32>(p.species), p.crowding);
  }
}
