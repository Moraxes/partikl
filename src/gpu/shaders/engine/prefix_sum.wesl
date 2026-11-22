// Wielopoziomowa suma prefiksowa (Blelloch) po `cell_counts`:
// bloki po 256 -> suma sum bloków (jedna grupa, do 256 bloków) -> dodanie przesunięć.

import super::uniforms::uniforms;
import super::bindings::{cell_counts, cell_offsets};

const BLOCK: u32 = 256u;
var<workgroup> temp: array<u32, 256>;

// Wykluczająca suma prefiksowa bloku; zwraca (wynik wątku, suma bloku).
fn block_scan(lid: u32, value: u32) -> vec2<u32> {
  temp[lid] = value;
  for (var d = 1u; d < BLOCK; d = d * 2u) {
    workgroupBarrier();
    if (lid + 1u) % (2u * d) == 0u {
      temp[lid] += temp[lid - d];
    }
  }
  workgroupBarrier();
  let total = temp[BLOCK - 1u];
  // Wszystkie wątki muszą przeczytać sumę, zanim ostatni ją wyzeruje.
  workgroupBarrier();
  if lid == BLOCK - 1u {
    temp[lid] = 0u;
  }
  for (var d = BLOCK / 2u; d >= 1u; d = d / 2u) {
    workgroupBarrier();
    if (lid + 1u) % (2u * d) == 0u {
      let t = temp[lid - d];
      temp[lid - d] = temp[lid];
      temp[lid] += t;
    }
  }
  workgroupBarrier();
  return vec2<u32>(temp[lid], total);
}

@compute @workgroup_size(256)
fn scan_blocks(@builtin(local_invocation_id) lid: vec3<u32>, @builtin(workgroup_id) wid: vec3<u32>) {
  let g = wid.x * BLOCK + lid.x;
  var value = 0u;
  if g < uniforms.num_cells {
    value = atomicLoad(&cell_counts[g]);
  }
  let r = block_scan(lid.x, value);
  if g < uniforms.num_cells {
    cell_offsets[g] = r.x;
  }
  if lid.x == 0u {
    cell_offsets[uniforms.num_cells + wid.x] = r.y;
  }
}

@compute @workgroup_size(256)
fn scan_sums(@builtin(local_invocation_id) lid: vec3<u32>) {
  var value = 0u;
  if lid.x < uniforms.num_blocks {
    value = cell_offsets[uniforms.num_cells + lid.x];
  }
  let r = block_scan(lid.x, value);
  if lid.x < uniforms.num_blocks {
    cell_offsets[uniforms.num_cells + lid.x] = r.x;
  }
}

@compute @workgroup_size(256)
fn add_offsets(@builtin(global_invocation_id) gid: vec3<u32>) {
  let g = gid.x;
  if g < uniforms.num_cells {
    cell_offsets[g] += cell_offsets[uniforms.num_cells + g / BLOCK];
  }
}
