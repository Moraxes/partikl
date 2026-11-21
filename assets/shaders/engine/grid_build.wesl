// Budowa siatki: zerowanie, histogram (atomicAdd), scatter w kolejności komórek.

import super::types::{Particle, cell_index};
import super::bindings::{uniforms, state, sorted, cell_counts, cell_cursors};

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
  var p = state[i];
  let dst = atomicAdd(&cell_cursors[cell_index(p.pos, uniforms)], 1u);
  p.id = i;
  sorted[dst] = p;
}
