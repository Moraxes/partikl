// Reguły diagnostyczne do testów silnika siatki (bez fizyki). Nic nie poruszają: dla każdej cząstki
// sumują (z wagą próbkowania) po sąsiadach w zasięgu oddziaływania liczbę sąsiadów, wektor `delta`
// i współczynnik `coeff`, a wynik zapisują w cząstce, skąd czyta go test:
// `crowding` = suma wag, `last` = suma wag · delta, `_pad` = bity f32 sumy wag · coeff.
// Ten sam kontrakt co `default_rules.wesl`: RuleAccum, accum_init, pair_interact, integrate_particle.

import super::super::engine::types::{Particle, Neighbour, RuleParams};

struct RuleAccum {
  count: f32,
  delta: vec2<f32>,
  coeff: f32,
}

fn accum_init() -> RuleAccum {
  return RuleAccum(0.0, vec2<f32>(0.0), 0.0);
}

fn pair_interact(self_p: Particle, other: Neighbour, delta: vec2<f32>, dist_sq: f32, coeff: f32, weight: f32, u: RuleParams, accum: ptr<function, RuleAccum>) {
  (*accum).count += weight;
  (*accum).delta += weight * delta;
  (*accum).coeff += weight * coeff;
}

fn integrate_particle(self_p: Particle, accum: RuleAccum, u: RuleParams) -> Particle {
  var p = self_p;
  p.crowding = accum.count;
  p.last = accum.delta;
  p._pad = bitcast<u32>(accum.coeff);
  return p;
}
