// Reguły diagnostyczne do testów silnika siatki (bez fizyki). Nic nie poruszają: dla każdej cząstki
// sumują (z wagą próbkowania) po sąsiadach w zasięgu oddziaływania liczbę sąsiadów, wektor `delta`
// i współczynnik `coeff`, a wynik zapisują w cząstce, skąd czyta go test:
// `crowding` = suma wag, `last` = suma wag · delta, `_pad` = bity f32 sumy wag · coeff.
// Ten sam kontrakt co `default_rules.wesl`; macierz sił czyta tak samo jak one (`matrix.wesl`).

import super::super::engine::types::{Particle, Neighbour};
import super::matrix::{Coeffs, load_coeffs, coeff_for};

struct RuleSelf {
  coeffs: Coeffs,
}

fn prepare_self(p: Particle) -> RuleSelf {
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

fn integrate_particle(self_p: Particle, accum: RuleAccum) -> Particle {
  var p = self_p;
  p.crowding = accum.count;
  p.last = accum.delta;
  p._pad = bitcast<u32>(accum.coeff);
  return p;
}
