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

//@option attraction key=G: mean, sum=ATTRACTION_SUM
//@option mix key=M: nw_vw, plain=MIX_PLAIN

import super::super::engine::types::Particle;
import super::bindings::rule_params;
import super::matrix::{Coeffs, load_coeffs, coeff_for};
import super::nodes as nd;

// Model sił. Wszystkie składowe są przyspieszeniami (j/s², masa 1), sumowanymi w `integrate_particle`:
//  - przyciąganie: macierz typów przez trójkątne jądro (`kernel_middle ± kernel_width`), wszystkie
//    gatunki; opcja `attraction`: `mean` = średnia ważona jądrem, `sum` = suma przez stałą wagę
//    odniesienia `attraction_sum_ref`; całość razy `attraction_scale`;
//  - separacja: sprężyny `separation_stiffness · (separation_radius − d)` od każdego sąsiada
//    (wszystkie gatunki) bliżej niż `separation_radius`, suma nasycana licznikiem
//    (`separation_saturation`): w tłumie siła rośnie wolniej niż liczba sąsiadów, więc dość silne
//    przyciąganie może ją przełamać;
//  - wyrównanie: relaksacja prędkości do średniej sąsiadów własnego gatunku w `vision_radius`
//    z szybkością `alignment_rate` (1/s);
//  - spójność: sprężyna do środka sąsiadów własnego gatunku w `vision_radius`, `cohesion_stiffness`
//    (1/s²);
//  - rozsuwanie nakładających się (`repulsion_radius`) to poprawka pozycji, nie siła.
// Opcja `mix`: `nw_vw` mnoży wyrównanie i spójność przez `g = sqrt(nw · vw)`, a przyciąganie dzieli
// przez `g` (`nw` udział własnego gatunku, `vw` rośnie z prędkością); `plain` = zwykła suma.

const F16_MAX: f32 = 65504.0;

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

// -- akumulatory (sumy ważone wagą próbkowania) --

// Przyciąganie: suma `coeff · h · dir` i suma wag jądra `h`.
struct AttractionAcc {
  force: vec2<f32>,
  kernel_w: f32,
}

// Separacja: suma sprężyn i liczba sąsiadów w `separation_radius`.
struct SeparationAcc {
  push: vec2<f32>,
  n: f32,
}

// Wyrównanie i spójność (własny gatunek w `vision_radius`): suma prędkości, suma przesunięć, liczba.
struct FlockAcc {
  vel: vec2<f32>,
  offset: vec2<f32>,
  n: f32,
}

// Rozsuwanie: suma poprawek pozycji i waga nakładających się par.
struct OverlapAcc {
  corr: vec2<f32>,
  n: f32,
}

// Sąsiedzi w zasięgu: własnego gatunku i wszyscy.
struct SpeciesAcc {
  same: f32,
  all: f32,
}

struct RuleAccum {
  attraction: AttractionAcc,
  separation: SeparationAcc,
  flock: FlockAcc,
  overlap: OverlapAcc,
  species: SpeciesAcc,
}

fn accum_init() -> RuleAccum {
  return RuleAccum(
    AttractionAcc(vec2(0.0), 0.0),
    SeparationAcc(vec2(0.0), 0.0),
    FlockAcc(vec2(0.0), vec2(0.0), 0.0),
    OverlapAcc(vec2(0.0), 0.0),
    SpeciesAcc(0.0, 0.0),
  );
}

// -- wkład pary --

fn attraction_pair(acc: AttractionAcc, g: nd::PairGeom, coeff: f32, weight: f32) -> AttractionAcc {
  let u = rule_params;
  let h = nd::hat(g.dist, u.kernel_middle, u.kernel_width);
  return AttractionAcc(acc.force + weight * coeff * h * g.dir, acc.kernel_w + weight * h);
}

fn separation_pair(acc: SeparationAcc, g: nd::PairGeom, weight: f32) -> SeparationAcc {
  let r = rule_params.separation_radius;
  if !nd::inside(g.dist, r) {
    return acc;
  }
  return SeparationAcc(acc.push + weight * nd::spring(g.dist, r, g.dir), acc.n + weight);
}

fn flock_pair(acc: FlockAcc, g: nd::PairGeom, delta: vec2<f32>, other_vel: vec2<f32>, weight: f32) -> FlockAcc {
  if !nd::inside(g.dist, rule_params.vision_radius) {
    return acc;
  }
  return FlockAcc(acc.vel + weight * other_vel, acc.offset + weight * delta, acc.n + weight);
}

fn overlap_pair(acc: OverlapAcc, g: nd::PairGeom, weight: f32) -> OverlapAcc {
  let reach = rule_params.repulsion_radius;
  if !nd::inside(g.dist, reach) {
    return acc;
  }
  // Poprawka bez wagi próbkowania, licznik z wagą (jak przed podziałem na akumulatory).
  return OverlapAcc(acc.corr + nd::overlap_push(g.dist, reach, g.dir), acc.n + weight);
}

fn species_pair(acc: SpeciesAcc, same: bool, weight: f32) -> SpeciesAcc {
  return SpeciesAcc(acc.same + weight * f32(same), acc.all + weight);
}

// Wkład jednego sąsiada (już w zasięgu oddziaływania, `dist_sq > 0`). `delta` wskazuje od
// `self_p` do `other` (po torusie; `other.pos` jest bez przesunięcia torusa), siłę typu sąsiada
// na typ `self_p` reguła czyta z macierzy (`rule_self.coeffs`), `weight` to waga próbkowania
// (1 bez próbkowania).
fn pair_interact(self_p: Particle, other: Neighbour, delta: vec2<f32>, dist_sq: f32, rule_self: RuleSelf, weight: f32, accum: ptr<function, RuleAccum>) {
  let g = nd::pair_geom(delta, dist_sq);
  let same = self_p.species == other.species;
  let coeff = coeff_for(rule_self.coeffs, other.species);
  (*accum).attraction = attraction_pair((*accum).attraction, g, coeff, weight);
  (*accum).separation = separation_pair((*accum).separation, g, weight);
  if same {
    (*accum).flock = flock_pair((*accum).flock, g, delta, other.vel, weight);
  }
  (*accum).overlap = overlap_pair((*accum).overlap, g, weight);
  (*accum).species = species_pair((*accum).species, same, weight);
}

// -- siły (po akumulacji) --

fn attraction_force(acc: AttractionAcc) -> vec2<f32> {
  let u = rule_params;
  @if(ATTRACTION_SUM) {
    return u.attraction_scale * acc.force / u.attraction_sum_ref;
  }
  @else {
    return u.attraction_scale * nd::shrunk_mean2(acc.force, acc.kernel_w, 2.0);
  }
}

fn separation_force(acc: SeparationAcc) -> vec2<f32> {
  let u = rule_params;
  return u.separation_stiffness * nd::saturating_sum2(acc.push, acc.n, u.separation_saturation);
}

fn alignment_force(acc: FlockAcc, vel: vec2<f32>) -> vec2<f32> {
  return nd::relax2(vel, nd::mean2(acc.vel, acc.n), rule_params.alignment_rate, acc.n > 0.0);
}

fn cohesion_force(acc: FlockAcc) -> vec2<f32> {
  return rule_params.cohesion_stiffness * nd::mean2(acc.offset, acc.n);
}

// Mnożnik mieszania boidów z przyciąganiem (opcja `mix`).
fn mix_gain(species: SpeciesAcc, vel: vec2<f32>) -> f32 {
  @if(MIX_PLAIN) {
    return 1.0;
  }
  @else {
    let nw = 0.1 + nd::share(species.same, species.all);
    let vw = log2(2.0 + dot(vel, vel));
    return sqrt(nw * vw);
  }
}

// Poprawka pozycji (rozsuwanie), tarcie kwadratowe i krok półjawnego Eulera (v, potem pos).
// Zawijanie robi silnik.
fn integrate_particle(self_p: Particle, accum: RuleAccum) -> Particle {
  let u = rule_params;
  var p = self_p;

  // Siły z prędkości sprzed poprawki rozsuwania.
  let gain = mix_gain(accum.species, p.vel);
  let flock = alignment_force(accum.flock, p.vel) + cohesion_force(accum.flock);
  let force = separation_force(accum.separation) + gain * flock + attraction_force(accum.attraction) / gain;

  // Poprawka tylko w prędkości; pozycja idzie za nią w tym samym kroku (p.pos += v * dt).
  let corr = nd::mean2(accum.overlap.corr, accum.overlap.n);
  p.vel += corr / u.dt;
  var v = nd::rest_below(p.vel, u.velocity_threshold);
  v = nd::quad_drag(nd::kick(v, force, u.dt), u.friction, u.dt);
  v = nd::drive(v, u.target_speed, u.drive_rate, u.dt);

  p.vel = v;
  p.pos += v * u.dt;
  return p;
}
