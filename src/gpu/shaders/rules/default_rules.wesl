// Reguły modelu (fizyka) wymienne bez ruszania silnika siatki.
// Opcje deklaruje nagłówek (`//@option`): użytkownik wybiera je klawiszem albo `--option`, a wybór
// włącza flagę shadera (nazwa po `=`); pierwszy wybór jest domyślny. Bez żadnej z dwóch pierwszych
// flag nakładania działa sprężyna.
//@option average_forces key=N: on=AVERAGE_FORCES, off
//@option overlap key=O: spring, resolve=OVERLAP_RESOLVE, adaptive_range=OVERLAP_ADAPTIVE
// Silnik podaje jeszcze flagi TYPES_AT_MOST_4 / TYPES_AT_MOST_8 (liczba typów, dla `matrix.wesl`).
// Kontrakt (silnik tylko to woła): RuleAccum, accum_init, RuleSelf, prepare_self, pair_interact,
// integrate_particle. Reguły same importują swoje wiązania (parametry, macierz sił).

import super::super::engine::types::{Particle, Neighbour};
import super::super::engine::bindings::rule_params;
import super::matrix::{Coeffs, load_coeffs, coeff_for};

// Dane własnej cząstki liczone raz na cząstkę, przed pętlą po sąsiadach.
struct RuleSelf {
  coeffs: Coeffs,
}

fn prepare_self(p: Particle) -> RuleSelf {
  return RuleSelf(load_coeffs(p.species));
}

struct RuleAccum {
  force: vec2<f32>,
  // Sąsiedzi w zasięgu (z wagą próbkowania).
  count: f32,
  // Rozsuwanie: suma poprawek pozycji i liczba nakładających się par.
  overlap_corr: vec2<f32>,
  overlap_count: f32,
}

fn accum_init() -> RuleAccum {
  return RuleAccum(vec2<f32>(0.0), 0.0, vec2<f32>(0.0), 0.0);
}

// Zasięg odpychania w trybie zmiennym dla wygładzonej liczby sąsiadów.
fn adaptive_radius(crowding: f32) -> f32 {
  let u = rule_params;
  return clamp(u.repulsion_radius * sqrt(crowding / u.adaptive_neighbours), u.repulsion_radius, u.adaptive_range_max);
}

// Wkład jednego sąsiada (już w zasięgu oddziaływania, `dist_sq > 0`). `delta` wskazuje od
// `self_p` do `other` (po torusie; `other.pos` jest bez przesunięcia torusa), `coeff` to
// siła typu sąsiada na typ `self_p`, `weight` waga próbkowania (1 bez próbkowania).
fn pair_interact(self_p: Particle, other: Neighbour, delta: vec2<f32>, dist_sq: f32, rule_self: RuleSelf, weight: f32, accum: ptr<function, RuleAccum>) {
  let u = rule_params;
  let coeff = coeff_for(rule_self.coeffs, other.species);
  let inv = inverseSqrt(dist_sq);
  let dist = dist_sq * inv;
  let dir = delta * inv;
  var reach = u.repulsion_radius;
@if(OVERLAP_ADAPTIVE) {
  reach = max(adaptive_radius(self_p.crowding), adaptive_radius(other.crowding));
}
  let attraction = coeff * max(0.0, 1.0 - abs((dist - u.kernel_middle) / u.kernel_width));
  var magnitude = 0.0;
@if(OVERLAP_RESOLVE) {
  // Bez sprężyny; przyciąganie zaczyna się dalej niż zasięg odpychania.
  magnitude = weight * attraction;
  if dist < reach {
    (*accum).overlap_corr += 0.5 * (dist - reach) * dir;
    (*accum).overlap_count += 1.0;
  }
}
@else {
  if dist < reach {
    magnitude = -u.repulsion_strength * weight * (reach - dist);
  } else {
    magnitude = weight * attraction;
  }
}
  (*accum).force += magnitude * dir;
@if(AVERAGE_FORCES || OVERLAP_ADAPTIVE) {
  (*accum).count += weight;
}
}

// Poprawka pozycji (rozsuwanie), tarcie kwadratowe i krok Verleta. Zawijanie robi silnik.
fn integrate_particle(self_p: Particle, accum: RuleAccum) -> Particle {
  let u = rule_params;
  var p = self_p;
  var force = accum.force;
@if(AVERAGE_FORCES) {
  if accum.count > 0.0 {
    force *= min(1.0, u.average_neighbours / accum.count);
  }
}
@if(OVERLAP_RESOLVE) {
  // Poprawka tylko w pozycji, więc staje się prędkością `poprawka / dt`.
  if accum.overlap_count > 0.0 {
    p.pos += accum.overlap_corr / accum.overlap_count;
  }
}
@if(OVERLAP_ADAPTIVE) {
  p.crowding += u.crowding_smoothing * (accum.count - p.crowding);
}
  let dt = u.dt;
  var displacement = p.pos - p.last;
  let velocity = displacement / dt;
  if dot(velocity, velocity) < u.velocity_threshold {
    displacement = vec2<f32>(0.0);
  }
  // Tarcie kwadratowe: dokładne rozwiązanie `v' = -c|v|v` na kroku.
  let free = displacement + force * dt * dt;
  let damped = free / (1.0 + u.friction * length(free));
  p.last = p.pos;
  p.pos = p.pos + damped;
  return p;
}
