// Krok symulacji: jeden wątek na cząstkę (model gather), sąsiedztwo 5x5 komórek z torusem
// liczonym na przebieg komórek, próbkowanie powyżej `max_candidates` kandydatów.

import super::types::{Particle, SimUniforms, cell_xy};
import super::bindings::{uniforms, state, sorted_ids, cell_counts, cell_offsets, force_matrix, load_neighbour};
import super::super::rules::default_rules::{RuleAccum, accum_init, pair_interact, integrate_particle};

const REACH: i32 = 2;

struct Run {
  start: u32,
  end: u32,
  shift: vec2<f32>,
  valid: bool,
}

// Przebieg kolejnych komórek wiersza `dy` względem komórki `cell`; `seg` 0/1 to części
// podzielone szwem torusa (część 1 istnieje tylko przy zawinięciu w osi x).
fn run_at(cell: vec2<i32>, dy: i32, seg: u32) -> Run {
  let dims = vec2<i32>(uniforms.grid_dims);
  var row = cell.y + dy;
  var shift = vec2<f32>(0.0);
  if row < 0 {
    row += dims.y;
    shift.y = -uniforms.world.y;
  } else if row >= dims.y {
    row -= dims.y;
    shift.y = uniforms.world.y;
  }
  let cx0 = cell.x - REACH;
  let cx1 = cell.x + REACH;
  var lo = cx0;
  var hi = cx1;
  if cx0 < 0 {
    if seg == 0u {
      lo = cx0 + dims.x;
      hi = dims.x - 1;
      shift.x = -uniforms.world.x;
    } else {
      lo = 0;
    }
  } else if cx1 >= dims.x {
    if seg == 0u {
      hi = dims.x - 1;
    } else {
      lo = 0;
      hi = cx1 - dims.x;
      shift.x = uniforms.world.x;
    }
  } else if seg == 1u {
    return Run(0u, 0u, shift, false);
  }
  let first = u32(row * dims.x + lo);
  let last = u32(row * dims.x + hi);
  return Run(cell_offsets[first], cell_offsets[last] + atomicLoad(&cell_counts[last]), shift, true);
}

// Zawinięcie pozycji o jeden rozmiar świata (najwyżej jedno zawinięcie na krok).
fn wrap(p: Particle) -> Particle {
  var q = p;
  let half = uniforms.world * 0.5;
  var adjust = vec2<f32>(0.0);
  if q.pos.x > half.x { adjust.x = -uniforms.world.x; } else if q.pos.x < -half.x { adjust.x = uniforms.world.x; }
  if q.pos.y > half.y { adjust.y = -uniforms.world.y; } else if q.pos.y < -half.y { adjust.y = uniforms.world.y; }
  q.pos += adjust;
  q.last += adjust;
  return q;
}

@compute @workgroup_size(64)
fn forces(@builtin(global_invocation_id) gid: vec3<u32>) {
  let j = gid.x;
  if j >= uniforms.num_particles {
    return;
  }
  let me = state[sorted_ids[j]];
  let cell = cell_xy(me.pos, uniforms);

  var candidates = 0u;
  for (var dy = -REACH; dy <= REACH; dy++) {
    for (var seg = 0u; seg < 2u; seg++) {
      let r = run_at(cell, dy, seg);
      if r.valid {
        candidates += r.end - r.start;
      }
    }
  }
  let every = max((candidates + uniforms.max_candidates - 1u) / uniforms.max_candidates, 1u);
  let offset = (sorted_ids[j] + uniforms.step_index) % every;
  let radius_sq = uniforms.interaction_radius * uniforms.interaction_radius;
  let weight = f32(every);

  var accum = accum_init();
  var k = 0u;
  for (var dy = -REACH; dy <= REACH; dy++) {
    for (var seg = 0u; seg < 2u; seg++) {
      let r = run_at(cell, dy, seg);
      if !r.valid {
        continue;
      }
      var s = r.start + (offset + every - k % every) % every;
      while s < r.end {
        let other = load_neighbour(s);
        let delta = other.pos + r.shift - me.pos;
        let dist_sq = dot(delta, delta);
        if dist_sq > 0.0 && dist_sq <= radius_sq {
          let coeff = force_matrix[other.species * uniforms.num_types + me.species];
          pair_interact(me, other, delta, dist_sq, coeff, weight, uniforms, &accum);
        }
        s += every;
      }
      k += r.end - r.start;
    }
  }
  state[sorted_ids[j]] = wrap(integrate_particle(me, accum, uniforms));
}
