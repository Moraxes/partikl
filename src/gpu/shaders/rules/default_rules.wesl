// Reguły modelu (fizyka) wymienne bez ruszania silnika siatki.
// Opcje deklaruje nagłówek (`//@option`): użytkownik wybiera je klawiszem albo `--option`, a wybór
// włącza flagę shadera (nazwa po `=`); pierwszy wybór jest domyślny. Bez żadnej z dwóch pierwszych
// flag nakładania działa sprężyna.
//@option average_forces key=N: on=AVERAGE_FORCES, off
//@option overlap key=O: spring, resolve=OVERLAP_RESOLVE, adaptive_range=OVERLAP_ADAPTIVE
// Silnik podaje jeszcze flagi TYPES_AT_MOST_4 / TYPES_AT_MOST_8 (liczba typów, dla `matrix.wesl`).
// Kontrakt (silnik tylko to woła):
//  - `Model`: pola modelu cząstki w osobnych buforach (silnik przenosi je z cząstką w scatterze i nie
//    zna ich pól); deklaracja `//@field` mówi Rustowi, ile to bajtów (WGSL musi mieć te same pola w
//    tej samej kolejności). Pola `shared` czytają też sąsiedzi, `private` tylko cząstka w całkowaniu.
//  - `Neighbour` (musi mieć `pos`), `pack_neighbour`, `unpack_neighbour`: co reguły widzą o sąsiedzie
//    w pętli sił; w pamięci jeden `vec4` na sąsiada, a pola `shared` mieszczą się w jednym słowie 4 B.
//  - `RuleAccum`, `accum_init`, `RuleSelf`, `prepare_self`, `pair_interact`, `integrate_particle`.
// Reguły same importują swoje wiązania (parametry, macierz sił z `rules/bindings.wesl`).

import super::super::engine::types::Particle;
import super::bindings::rule_params;
import super::matrix::{Coeffs, load_coeffs, coeff_for};

//@field crowding: f32 shared
// Wygładzona liczba sąsiadów (tryb zmiennego zasięgu).
struct Model {
  crowding: f32,
}

struct Neighbour {
  pos: vec2<f32>,
  species: u32,
  crowding: f32,
}

// Układ `sorted_hot[slot]`: (pos.x, pos.y, bity species, crowding).
fn pack_neighbour(p: Particle, m: Model) -> vec4<f32> {
  return vec4<f32>(p.pos, bitcast<f32>(p.species), m.crowding);
}

fn unpack_neighbour(v: vec4<f32>) -> Neighbour {
  return Neighbour(v.xy, bitcast<u32>(v.z), v.w);
}

// Dane własnej cząstki liczone raz na cząstkę, przed pętlą po sąsiadach.
struct RuleSelf {
  coeffs: Coeffs,
  crowding: f32,
}

fn prepare_self(p: Particle, m: Model) -> RuleSelf {
  return RuleSelf(load_coeffs(p.species), m.crowding);
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
// `self_p` do `other` (po torusie; `other.pos` jest bez przesunięcia torusa), siłę typu sąsiada
// na typ `self_p` reguła czyta z macierzy (`rule_self.coeffs`), `weight` to waga próbkowania
// (1 bez próbkowania).
fn pair_interact(self_p: Particle, other: Neighbour, delta: vec2<f32>, dist_sq: f32, rule_self: RuleSelf, weight: f32, accum: ptr<function, RuleAccum>) {
  let u = rule_params;
  let coeff = coeff_for(rule_self.coeffs, other.species);
  let inv = inverseSqrt(dist_sq);
  let dist = dist_sq * inv;
  let dir = delta * inv;
  var reach = u.repulsion_radius;
@if(OVERLAP_ADAPTIVE) {
  reach = max(adaptive_radius(rule_self.crowding), adaptive_radius(other.crowding));
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
fn integrate_particle(self_p: Particle, m: ptr<function, Model>, accum: RuleAccum) -> Particle {
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
    let corr = accum.overlap_corr / accum.overlap_count;
    p.pos += corr;
    p.vel += corr / u.dt;
  }
}
@if(OVERLAP_ADAPTIVE) {
  (*m).crowding += u.crowding_smoothing * (accum.count - (*m).crowding);
}
  let dt = u.dt;
  var v = p.vel;
  if dot(p.vel, p.vel) < u.velocity_threshold {
    v = vec2<f32>(0.0);
  }
  let v_free = v + force * dt;
  // Tarcie kwadratowe: dokładne rozwiązanie `v' = -c|v|v` na kroku.
  let v_damped = v_free / (1.0 + u.friction * length(v_free) * dt);
  p.vel = v_damped;
  p.pos += v_damped * dt;
  return p;
}
