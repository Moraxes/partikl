// Krok symulacji: jeden wątek na cząstkę (model gather), sąsiedztwo 5x5 komórek z torusem
// liczonym na przebieg komórek, próbkowanie powyżej `max_candidates` kandydatów.

import super::types::{Particle, Neighbour, SimUniforms, cell_xy};
import super::bindings::{uniforms, state, sorted_state, cell_counts, cell_offsets, force_matrix, load_neighbour};
// Zestaw reguł wybiera flaga shadera (`RuleSet` w Rust): domyślne reguły fizyki albo diagnostyczne
// (testy silnika siatki).
@if(RULES_DIAGNOSTIC) import super::super::rules::diagnostic_rules::{RuleAccum, accum_init, pair_interact, integrate_particle};
@if(!RULES_DIAGNOSTIC) import super::super::rules::default_rules::{RuleAccum, accum_init, pair_interact, integrate_particle};

const REACH: i32 = 2;

// Współczynniki sił dla typu własnego `species`: `coeff_for(c, other.species)` to siła, z jaką
// sąsiad tego typu działa na cząstkę. Odczyt macierzy w pętli po sąsiadach (zależny od typu
// sąsiada) kosztował ok. 20% przebiegu sił, więc dla małej liczby typów kolumna macierzy leży w
// rejestrach (warianty shadera wg flag `TYPES_AT_MOST_4` / `TYPES_AT_MOST_8`).
@if(TYPES_AT_MOST_4) {
  alias Coeffs = vec4<f32>;

  fn load_coeffs(species: u32) -> Coeffs {
    var c = vec4<f32>(0.0);
    for (var t = 0u; t < uniforms.num_types; t++) {
      c[t] = force_matrix[t * uniforms.num_types + species];
    }
    return c;
  }

  fn coeff_for(c: Coeffs, other: u32) -> f32 {
    return c[other];
  }
}
@elif(TYPES_AT_MOST_8) {
  struct Coeffs {
    lo: vec4<f32>,
    hi: vec4<f32>,
  }

  fn load_coeffs(species: u32) -> Coeffs {
    var c = Coeffs(vec4<f32>(0.0), vec4<f32>(0.0));
    for (var t = 0u; t < uniforms.num_types; t++) {
      let v = force_matrix[t * uniforms.num_types + species];
      if t < 4u { c.lo[t] = v; } else { c.hi[t - 4u] = v; }
    }
    return c;
  }

  fn coeff_for(c: Coeffs, other: u32) -> f32 {
    return select(c.lo[other & 3u], c.hi[other & 3u], other >= 4u);
  }
}
@else {
  struct Coeffs {
    species: u32,
  }

  fn load_coeffs(species: u32) -> Coeffs {
    return Coeffs(species);
  }

  fn coeff_for(c: Coeffs, other: u32) -> f32 {
    return force_matrix[other * uniforms.num_types + c.species];
  }
}

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

// Jeden kandydat: w zasięgu oddziaływania wywołuje regułę `pair_interact`. `shift` to przesunięcie
// torusa biegu komórek kandydata.
fn visit(me: Particle, other: Neighbour, shift: vec2<f32>, radius_sq: f32, weight: f32, coeffs: Coeffs, accum: ptr<function, RuleAccum>) {
  let delta = other.pos + shift - me.pos;
  let dist_sq = dot(delta, delta);
  // 0 < dist_sq <= radius_sq jednym porównaniem całkowitym: dla nieujemnych liczb zmiennoprzecinkowych
  // kolejność bitów jest kolejnością wartości, a 0 po odjęciu 1 przechodzi w największą liczbę
  // (NaN daje fałsz, jak zwykłe porównanie).
  if bitcast<u32>(dist_sq) - 1u < bitcast<u32>(radius_sq) {
    pair_interact(me, other, delta, dist_sq, coeff_for(coeffs, other.species), weight, uniforms, accum);
  }
}

@compute @workgroup_size(64)
fn forces(@builtin(global_invocation_id) gid: vec3<u32>) {
  let j = gid.x;
  if j >= uniforms.num_particles {
    return;
  }
  let me = sorted_state[j];
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

  let coeffs = load_coeffs(me.species);
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
        visit(me, o0, r.shift, radius_sq, weight, coeffs, &accum);
        visit(me, o1, r.shift, radius_sq, weight, coeffs, &accum);
        visit(me, o2, r.shift, radius_sq, weight, coeffs, &accum);
        visit(me, o3, r.shift, radius_sq, weight, coeffs, &accum);
        s += 4u * every;
      }
      while s < r.end {
        visit(me, load_neighbour(s), r.shift, radius_sq, weight, coeffs, &accum);
        s += every;
      }
      k += r.end - r.start;
    }
  }
  // Wynik na miejsce `j`: stan zostaje w kolejności siatki.
  state[j] = wrap(integrate_particle(me, accum, uniforms));
}
