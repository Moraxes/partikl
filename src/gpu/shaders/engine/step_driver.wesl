// Krok symulacji: jeden wątek na cząstkę (model gather), sąsiedztwo 5x5 komórek z torusem
// liczonym na przebieg komórek, próbkowanie powyżej `max_candidates` kandydatów.

import super::types::{Particle, cell_xy, cell_index};
import super::uniforms::uniforms;
import super::bindings::{state, model, sorted_state, sorted_model, cell_counts, cell_offsets, next_counts, cell_ranks, load_neighbour};
// Reguły: zawsze ten sam moduł `rules::model`; jego źródło wybiera Rust (`RuleSource`: wbudowane,
// diagnostyczne albo plik `--rules`). Kontrakt w nagłówku `rules/default_rules.wesl`.
import super::super::rules::model::{Neighbour, RuleAccum, RuleSelf, accum_init, prepare_self, pair_interact, integrate_particle};

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
  return q;
}

// Jeden kandydat: w zasięgu oddziaływania wywołuje regułę `pair_interact` (z danymi własnej cząstki
// z `prepare_self`, liczonymi raz na cząstkę). `shift` to przesunięcie
// torusa biegu komórek kandydata.
fn visit(me: Particle, other: Neighbour, shift: vec2<f32>, radius_sq: f32, weight: f32, rule_self: RuleSelf, accum: ptr<function, RuleAccum>) {
  let delta = other.pos + shift - me.pos;
  let dist_sq = dot(delta, delta);
  // 0 < dist_sq <= radius_sq jednym porównaniem całkowitym: dla nieujemnych liczb zmiennoprzecinkowych
  // kolejność bitów jest kolejnością wartości, a 0 po odjęciu 1 przechodzi w największą liczbę
  // (NaN daje fałsz, jak zwykłe porównanie).
  if bitcast<u32>(dist_sq) - 1u < bitcast<u32>(radius_sq) {
    pair_interact(me, other, delta, dist_sq, rule_self, weight, accum);
  }
}

@compute @workgroup_size(64)
fn forces(@builtin(global_invocation_id) gid: vec3<u32>) {
  let j = gid.x;
  if j >= uniforms.num_particles {
    return;
  }
  let me = sorted_state[j];
  var my_model = sorted_model[j];
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
  let offset = (me.id + uniforms.step_index) % every;
  let radius_sq = uniforms.interaction_radius * uniforms.interaction_radius;
  let weight = f32(every);

  let rule_self = prepare_self(me, my_model);
  var accum = accum_init();
  var k = 0u;
  for (var dy = -REACH; dy <= REACH; dy++) {
    for (var seg = 0u; seg < 2u; seg++) {
      let r = run_at(cell, dy, seg);
      if !r.valid {
        continue;
      }
      var s = r.start + (offset + every - k % every) % every;
      // Po 4 kandydatów: odczyty wyprzedzają obliczenia. Pętla czeka na opóźnienie zależnych
      // odczytów (odczyt → test zasięgu → gałąź), więc kilka niezależnych odczytów naraz skraca
      // przebieg o ok. 17–20%; rozwinięcie ×2 daje mniej, ×8 prawie nie więcej. Reszta biegu
      // (do 3 kandydatów) idzie pojedynczo.
      while s + 3u * every < r.end {
        let o0 = load_neighbour(s);
        let o1 = load_neighbour(s + every);
        let o2 = load_neighbour(s + 2u * every);
        let o3 = load_neighbour(s + 3u * every);
        visit(me, o0, r.shift, radius_sq, weight, rule_self, &accum);
        visit(me, o1, r.shift, radius_sq, weight, rule_self, &accum);
        visit(me, o2, r.shift, radius_sq, weight, rule_self, &accum);
        visit(me, o3, r.shift, radius_sq, weight, rule_self, &accum);
        s += 4u * every;
      }
      while s < r.end {
        visit(me, load_neighbour(s), r.shift, radius_sq, weight, rule_self, &accum);
        s += every;
      }
      k += r.end - r.start;
    }
  }
  // Wynik na miejsce `j`: stan zostaje w kolejności siatki.
  let next = wrap(integrate_particle(me, &my_model, accum));
  state[j] = next;
  // Histogram następnego kroku z nowej pozycji; ranga wskaże miejsce w komórce przy scatterze.
  cell_ranks[j] = atomicAdd(&next_counts[cell_index(next.pos, uniforms)], 1u);
  model[j] = my_model;
}
