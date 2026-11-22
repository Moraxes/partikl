// Statystyki stanu po kroku (opcja, `GpuStats`): jeden przebieg na klatkę po siłach. Każda grupa
// (64 cząstki) redukuje wartości w pamięci grupy i zapisuje po jednym słowie na statystykę:
// - `stats[0]`: liczba cząstek z nieskończonym albo NaN w `pos` lub `last` (atomicAdd),
// - `stats[1]`: maksymalna prędkość |pos − last| / dt po cząstkach skończonych (atomicMax na bitach
//   f32; prędkość jest nieujemna, więc kolejność bitów to kolejność wartości),
// - `stats[2 + g]`: suma 0.5·|v|² grupy `g` (bity f32, jedno miejsce na grupę). Sumę po grupach liczy
//   CPU w f64: WGSL nie ma atomowej sumy f32, a stałoprzecinkowa suma u32 przepełniałaby się.

import super::types::Particle;
import super::uniforms::uniforms;
import super::bindings::{state, stats};
import super::super::rules::bindings::rule_params;

var<workgroup> red_nan: array<u32, 64>;
var<workgroup> red_speed: array<f32, 64>;
var<workgroup> red_energy: array<f32, 64>;

// Liczba f32 skończona: wykładnik nie jest same jedynki (NaN i nieskończoność mają je wszystkie).
// Sprawdzane bitowo, bo `isNan`/`isInf` mogą nie być obsługiwane przez naga.
fn is_finite(x: f32) -> bool {
  return (bitcast<u32>(x) & 0x7f800000u) != 0x7f800000u;
}

fn finite2(v: vec2<f32>) -> bool {
  return is_finite(v.x) && is_finite(v.y);
}

@compute @workgroup_size(64)
fn reduce_stats(
  @builtin(global_invocation_id) gid: vec3<u32>,
  @builtin(local_invocation_index) lid: u32,
  @builtin(workgroup_id) grp: vec3<u32>,
) {
  let i = gid.x;
  var nan = 0u;
  var speed = 0.0;
  var energy = 0.0;
  // Wątki poza zakresem zostają z zerami, żeby redukcja miała stałą liczbę kroków.
  if i < uniforms.num_particles {
    let p = state[i];
    if finite2(p.pos) && finite2(p.last) {
      // Torus: różnicę zawijamy o rozmiar świata, tak jak `wrap` w `step_driver.wesl`.
      let d = p.pos - p.last;
      let dw = d - uniforms.world * round(d / uniforms.world);
      let v = dw / rule_params.dt;
      let s = length(v);
      let e = 0.5 * dot(v, v);
      // Prędkość lub energia mogą przepełnić f32 (nieskończoność): taka cząstka nie wchodzi do max i sumy.
      if is_finite(s) && is_finite(e) {
        speed = s;
        energy = e;
      }
    } else {
      nan = 1u;
    }
  }

  red_nan[lid] = nan;
  red_speed[lid] = speed;
  red_energy[lid] = energy;
  workgroupBarrier();
  for (var stride = 32u; stride > 0u; stride = stride / 2u) {
    if lid < stride {
      red_nan[lid] += red_nan[lid + stride];
      red_speed[lid] = max(red_speed[lid], red_speed[lid + stride]);
      red_energy[lid] += red_energy[lid + stride];
    }
    workgroupBarrier();
  }

  if lid == 0u {
    atomicAdd(&stats[0], red_nan[0]);
    atomicMax(&stats[1], bitcast<u32>(red_speed[0]));
    atomicStore(&stats[2u + grp.x], bitcast<u32>(red_energy[0]));
  }
}
