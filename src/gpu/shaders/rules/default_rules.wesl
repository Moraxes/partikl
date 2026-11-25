// Reguły modelu (fizyka) wymienne bez ruszania silnika siatki.
// Silnik podaje jeszcze flagi TYPES_AT_MOST_4 / TYPES_AT_MOST_8 (liczba typów, dla `matrix.wesl`).
// Kontrakt (silnik tylko to woła):
//  - Opcjonalnie `Model`: pola modelu cząstki w osobnych buforach (silnik przenosi je z cząstką w
//    scatterze i nie zna ich pól); deklaracja `//@field` mówi Rustowi, ile to bajtów (WGSL musi mieć
//    te same pola w tej samej kolejności). Pola `shared` czytają też sąsiedzi, `private` tylko
//    cząstka w całkowaniu. Gdy reguły nie mają `//@field`, nie definiują `Model`, a `pack_neighbour`,
//    `prepare_self` i `integrate_particle` pomijają parametr modelu.
//  - `Neighbour` (musi mieć `pos`), `pack_neighbour`, `unpack_neighbour`: co reguły widzą o sąsiedzie
//    w pętli sił; w pamięci jeden `vec4` na sąsiada, a pola `shared` mieszczą się w jednym słowie 4 B.
//  - `RuleAccum`, `accum_init`, `RuleSelf`, `prepare_self`, `pair_interact`, `integrate_particle`.
// Reguły same importują swoje wiązania (parametry, macierz sił z `rules/bindings.wesl`).

import super::super::engine::types::Particle;
import super::bindings::rule_params;
import super::matrix::{Coeffs, load_coeffs, coeff_for};
import super::nodes as nd;

const RULE_SEPARATION: u32 = 0u;
const RULE_ALIGNMENT: u32 = 1u;
const RULE_COHESION: u32 = 2u;
const F16_MAX: f32 = 65504.0;

fn pair_weight(rule: u32, self_species: u32, other_species: u32) -> f32 {
  let u = rule_params;
  switch rule {
    case RULE_SEPARATION: {
      return u.separation_weight;
    }
    case RULE_ALIGNMENT: {
      return u.alignment_weight;
    }
    default: {
      return u.coherence_weight;
    }
  }
}

struct Neighbour {
  pos: vec2<f32>,
  vel: vec2<f32>,
  species: u32,
}

// Układ `sorted_hot[slot]`: (pos.x, pos.y, vel.xy, species).
fn pack_neighbour(p: Particle) -> vec4<f32> {
  // Zakres binary16 to [-65504, 65504]; większe składowe dają ±Inf i propagują NaN u sąsiadów.
  let vel = clamp(p.vel, vec2(-F16_MAX), vec2(F16_MAX));
  return vec4<f32>(p.pos, bitcast<f32>(pack2x16float(vel)), bitcast<f32>(p.species));
}

fn unpack_neighbour(v: vec4<f32>) -> Neighbour {
  return Neighbour(v.xy, unpack2x16float(bitcast<u32>(v.z)), bitcast<u32>(v.w));
}

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
  neighbours: f32,
  same_species_neighbours: f32,
  kernel_w: f32,
  // Rozsuwanie: suma poprawek pozycji i liczba nakładających się par.
  overlap_corr: vec2<f32>,
  overlap_count: f32,
  
  // boidy
  sep: vec2<f32>,
  sep_n: f32,
  align: vec2<f32>,
  align_w: f32,
  cohere: vec2<f32>,
  view_n: f32,
}

fn accum_init() -> RuleAccum {
  return RuleAccum(
    vec2(0.0), 0.0, 0.0, 0.0,
    vec2(0.0), 0.0,
    vec2(0.0), 0.0,
    vec2(0.0), 0.0,
    vec2(0.0), 0.0,
  );
}

// Wkład jednego sąsiada (już w zasięgu oddziaływania, `dist_sq > 0`). `delta` wskazuje od
// `self_p` do `other` (po torusie; `other.pos` jest bez przesunięcia torusa), siłę typu sąsiada
// na typ `self_p` reguła czyta z macierzy (`rule_self.coeffs`), `weight` to waga próbkowania
// (1 bez próbkowania).
fn pair_interact(self_p: Particle, other: Neighbour, delta: vec2<f32>, dist_sq: f32, rule_self: RuleSelf, weight: f32, accum: ptr<function, RuleAccum>) {
  let u = rule_params;
  let reach = u.repulsion_radius;
  let g = nd::pair_geom(delta, dist_sq);
  let h = nd::hat(g.dist, u.kernel_middle, u.kernel_width);
  let attraction = h * coeff_for(rule_self.coeffs, other.species);
  // Przyciąganie zaczyna się dalej niż zasięg odpychania.
  (*accum).force += weight * attraction * g.dir;
  (*accum).kernel_w += weight * h;
  if nd::inside(g.dist, reach) {
    (*accum).overlap_corr += nd::overlap_push(g.dist, reach, g.dir);
    (*accum).overlap_count += 1.0;
  }
  let same_species = self_p.species == other.species;
  (*accum).same_species_neighbours += weight * f32(same_species);
  (*accum).neighbours += weight;

  // boidy
  if same_species && g.dist < u.separation_radius {
    let w = pair_weight(RULE_SEPARATION, self_p.species, other.species);
    (*accum).sep += weight * w * nd::separation(g.dir, g.dist);
    (*accum).sep_n += weight;
  }
  if same_species && g.dist < u.vision_radius {
    let wa = pair_weight(RULE_ALIGNMENT, self_p.species, other.species);
    let wc = pair_weight(RULE_COHESION, self_p.species, other.species);
    (*accum).align += weight * wa * other.vel;
    (*accum).align_w += weight * wa;
    (*accum).cohere += weight * wc * delta;
    (*accum).view_n += weight;
  }
}

// Poprawka pozycji (rozsuwanie), tarcie kwadratowe i krok półjawnego Eulera (v, potem pos).
// Zawijanie robi silnik.
fn integrate_particle(self_p: Particle, accum: RuleAccum) -> Particle {
  let u = rule_params;
  var p = self_p;

  // średni wektor coeff * kierunek ważony kernelem przyciągania
  let attraction = accum.force / (accum.kernel_w + 2.0);

  let n = accum.view_n;
  let a_sep = nd::mean2(accum.sep, accum.sep_n);
  let a_align = nd::mean2(accum.align - p.vel * accum.align_w, n);
  let a_coh = nd::mean2(accum.cohere, n);
  let boid = a_sep + a_align + a_coh;
 
  let nw = 0.1 + accum.same_species_neighbours / max(1.0, accum.neighbours);
  let vw = log2(2.0 + dot(p.vel, p.vel));
  let force = sqrt(nw * vw) * boid + 10.0 * attraction / sqrt(nw * vw);

  // Poprawka tylko w prędkości; pozycja idzie za nią w tym samym kroku (p.pos += v * dt).
  let corr = nd::mean2(accum.overlap_corr, accum.overlap_count);
  p.vel += corr / u.dt;
  var v = nd::rest_below(p.vel, u.velocity_threshold);
  v = nd::quad_drag(nd::kick(v, force, u.dt), u.friction, u.dt);
  v = nd::drive(v, u.target_speed, u.drive_rate, u.dt);

  p.vel = v;
  p.pos += v * u.dt;
  return p;
}
