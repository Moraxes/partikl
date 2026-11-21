// Reguły diagnostyczne do testów silnika siatki (bez fizyki). Nic nie poruszają: dla każdej cząstki
// sumują (z wagą próbkowania) po sąsiadach w zasięgu oddziaływania liczbę sąsiadów, wektor `delta`
// i współczynnik `coeff`, a wynik zapisują w cząstce i jej modelu, skąd czyta go test:
// `last` = suma wag · delta, model: `count` = suma wag, `coeff` = suma wag · coeff.
// Ten sam kontrakt co `default_rules.wesl`; macierz sił czyta tak samo jak one (`matrix.wesl`).

import super::super::engine::types::Particle;
import super::matrix::{Coeffs, load_coeffs, coeff_for};

//@field count: f32
//@field coeff: f32
struct Model {
  count: f32,
  coeff: f32,
}

struct Neighbour {
  pos: vec2<f32>,
  species: u32,
}

fn pack_neighbour(p: Particle, m: Model) -> vec4<f32> {
  return vec4<f32>(p.pos, bitcast<f32>(p.species), 0.0);
}

fn unpack_neighbour(v: vec4<f32>) -> Neighbour {
  return Neighbour(v.xy, bitcast<u32>(v.z));
}

struct RuleSelf {
  coeffs: Coeffs,
}

fn prepare_self(p: Particle, m: Model) -> RuleSelf {
  return RuleSelf(load_coeffs(p.species));
}

struct RuleAccum {
  count: f32,
  delta: vec2<f32>,
  coeff: f32,
}

fn accum_init() -> RuleAccum {
  return RuleAccum(0.0, vec2<f32>(0.0), 0.0);
}

fn pair_interact(self_p: Particle, other: Neighbour, delta: vec2<f32>, dist_sq: f32, rule_self: RuleSelf, weight: f32, accum: ptr<function, RuleAccum>) {
  let coeff = coeff_for(rule_self.coeffs, other.species);
  (*accum).count += weight;
  (*accum).delta += weight * delta;
  (*accum).coeff += weight * coeff;
}

fn integrate_particle(self_p: Particle, m: ptr<function, Model>, accum: RuleAccum) -> Particle {
  var p = self_p;
  p.last = accum.delta;
  (*m).count = accum.count;
  (*m).coeff = accum.coeff;
  return p;
}
