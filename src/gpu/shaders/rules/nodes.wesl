// 1. Czyste funkcje, bez czytania rule_params itp.
// 2. Trzy domeny
//      - para: (dist, dir, sąsiad) -> f32 | vec2<f32>
//      - post: po akumulacji sił
//      - całkowanie: (prędkość, pozycja, siła) -> vec2<f32>
// 3. Akumulacja to zawsze ważona suma. Licznik też jest sumą (wag albo jedynek).
//    Normalizacja to osobny węzeł post, który dostaje (suma, licznik) jawnie.
//    Kolejność operacji jest wtedy widoczna w kodzie, a nie ukryta.

// -- para --
struct PairGeom {
  dist: f32,
  dir: vec2<f32>,
}

fn pair_geom(delta: vec2<f32>, dist_sq: f32) -> PairGeom {
  let inv = inverseSqrt(dist_sq);
  return PairGeom(dist_sq * inv, delta * inv);
}

fn hat(x: f32, middle: f32, width: f32) -> f32 {
  return max(0.0, 1.0 - abs((x - middle) / width));
}

fn inside(dist: f32, reach: f32) -> bool {
  return dist < reach;
}

// Sprężyna o długości spoczynkowej `rest` wzdłuż `dir` (od cząstki do sąsiada): bliżej niż `rest`
// wskazuje od sąsiada, dalej do niego; długość = wydłużenie.
fn spring(dist: f32, rest: f32, dir: vec2<f32>) -> vec2<f32> {
  return (dist - rest) * dir;
}

fn overlap_push(dist: f32, reach: f32, dir: vec2<f32>) -> vec2<f32> {
  return 0.5 * spring(dist, reach, dir);
}

// -- gate --

fn in_view(self_heading: vec2<f32>, dir: vec2<f32>, cos_half_fov: f32) -> bool {
  return dot(self_heading, dir) > cos_half_fov;
}

// -- post --

fn mean2(sum: vec2<f32>, count: f32) -> vec2<f32> {
  return select(vec2(0.0), sum / count, count > 0.0);
}

fn capped_mean2(sum: vec2<f32>, count: f32, n0: f32) -> vec2<f32> {
  return select(sum, sum * min(1.0, n0 / count), count > 0.0);
}

// Średnia z wagą `prior` dodaną do mianownika (ściąga do zera przy małej łącznej wadze).
fn shrunk_mean2(sum: vec2<f32>, weight: f32, prior: f32) -> vec2<f32> {
  return sum / (weight + prior);
}

// Suma nasycana licznikiem: dla `count << n0` to suma, dla `count >> n0` średnia razy `n0`.
fn saturating_sum2(sum: vec2<f32>, count: f32, n0: f32) -> vec2<f32> {
  return sum / (1.0 + count / n0);
}

// Udział części w całości (0 bez całości).
fn share(part: f32, total: f32) -> f32 {
  return part / max(1.0, total);
}

// Relaksacja `current` do `goal` z szybkością `rate` (1/s): przyspieszenie, 0 bez celu.
fn relax2(current: vec2<f32>, goal: vec2<f32>, rate: f32, has_goal: bool) -> vec2<f32> {
  return select(vec2(0.0), rate * (goal - current), has_goal);
}

fn smoothed(prev: f32, tgt: f32, rate: f32) -> f32 {
  return prev + rate * (tgt - prev);
}

fn drive(v: vec2<f32>, target_speed: f32, rate: f32, dt: f32) -> vec2<f32> {
  let speed = length(v);
  if speed < 1e-3 {
    return v;
  }
  return v + (v / speed * (target_speed - speed) * rate * dt);
}

// -- całkowanie --

fn rest_below(v: vec2<f32>, threshold_sq: f32) -> vec2<f32> {
  return select(v, vec2(0.0), dot(v, v) < threshold_sq);
}

fn kick(v: vec2<f32>, force: vec2<f32>, dt: f32) -> vec2<f32> {
  return v + force * dt;
}

fn quad_drag(v: vec2<f32>, c: f32, dt: f32) -> vec2<f32> {
  return v / (1.0 + c * dt * length(v));
}

// -- misc --

// sigmoida oparta na inverse sqrt o tej samej pochodnej w 0 co logistyczna
// 0.5 + 0.25 x / sqrt(1 + 0.25 x^2)
fn sigmoid_rsqrt_01(x: f32) -> f32 {
  return 0.25 * x * inverseSqrt(1.0 + 0.25 * x * x) + 0.5;
}

fn sigmoid_rsqrt_11(x: f32) -> f32 {
  return 0.5 * x * inverseSqrt(1.0 + 0.25 * x * x);
}

// |a x b| = |a||b||sin(theta_ab)
fn cross2(a: vec2<f32>, b: vec2<f32>) -> f32 {
  return a.x * b.y - a.y * b.x;
}