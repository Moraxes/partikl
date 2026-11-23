// Reguły modelu (fizyka) wymienne bez ruszania silnika siatki.
// Opcje deklaruje nagłówek (`//@option`): użytkownik wybiera je klawiszem albo `--option`, a wybór
// włącza flagę shadera (nazwa po `=`); pierwszy wybór jest domyślny. Bez żadnej z dwóch pierwszych
// flag nakładania działa sprężyna (wybór `spring`); domyślnie jest to rozsuwanie `resolve`.
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
import super::nodes as nd;

//@field phantom_: f32
struct Model {
  phantom_: f32,
}

struct Neighbour {
  pos: vec2<f32>,
  vel: vec2<f32>,
  species: u32,
}

fn pack_neighbour(p: Particle, m: Model) -> vec4<f32> {
  return vec4<f32>(p.pos, bitcast<f32>(pack2x16float(p.vel)), bitcast<f32>(p.species));
}

fn unpack_neighbour(v: vec4<f32>) -> Neighbour {
  return Neighbour(v.xy, unpack2x16float(bitcast<u32>(v.z)), bitcast<u32>(v.w));
}

// Dane własnej cząstki liczone raz na cząstkę, przed pętlą po sąsiadach.
struct RuleSelf {
  vel_norm: vec2<f32>,
}

fn prepare_self(p: Particle, m: Model) -> RuleSelf {
  let len = length(p.vel);
  return RuleSelf(select(vec2(0.0), p.vel / len, len > 0));
}

struct RuleAccum {
  sep: vec2<f32>,
  align: vec2<f32>,
  cohere: vec2<f32>,
  view_n: f32,
}

fn accum_init() -> RuleAccum {
  return RuleAccum(vec2(0.0), vec2(0.0), vec2(0.0), 0.0);
}

fn pair_interact(
  self_p: Particle,
  other: Neighbour,
  delta: vec2<f32>,
  dist_sq: f32,
  rule_self: RuleSelf,
  weight: f32,
  accum: ptr<function, RuleAccum>,
) {
  let u = rule_params;
  let g = nd::pair_geom(delta, dist_sq);
  if g.dist < u.separation_radius {
    (*accum).sep += weight * nd::separation(g.dir, g.dist);
  }
  // if dist < u.vision_radius && nd::in_view(rule_self.vel_norm, g.dir, u.fov_cos) {
  if g.dist < u.vision_radius {
    (*accum).align += weight * other.vel;
    (*accum).cohere += weight * delta;
    (*accum).view_n += weight;
  }
}

fn integrate_particle(self_p: Particle, m: ptr<function, Model>, accum: RuleAccum) -> Particle {
  let u = rule_params;
  var p = self_p;
  let a_sep = u.separation_weight * accum.sep;
  let a_align = u.alignment_weight * nd::mean2(accum.align - p.vel * accum.view_n, accum.view_n);
  let a_coh = u.coherence_weight * nd::mean2(accum.cohere, accum.view_n);
  p.vel = nd::kick(p.vel, a_sep + a_align + a_coh, u.dt);
  p.vel = nd::drive(p.vel, u.target_speed, u.drive_rate, u.dt);
  p.pos += p.vel * u.dt;
  return p;
}
