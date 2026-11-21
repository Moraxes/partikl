// Typy wspólne silnika siatki i reguł. Układ musi zgadzać się z `src/gpu/mod.rs`.

// Cząstka silnika: tylko to, czego potrzebuje siatka i render. Pola modelu (np. `crowding`) leżą w
// osobnych buforach deklarowanych przez reguły (`Model`, patrz `rules/default_rules.wesl`).
struct Particle {
  pos: vec2<f32>,
  // Pozycja z poprzedniego kroku (całkowanie Verleta).
  last: vec2<f32>,
  species: u32,
  // Stały identyfikator cząstki (stan jest w kolejności siatki, nie w kolejności id).
  id: u32,
}

// Stałe i geometria kroku (silnik siatki). Układ jak `EngineUniforms` w `src/gpu/compute/mod.rs`.
struct EngineUniforms {
  world: vec2<f32>,
  cell_size: vec2<f32>,
  grid_dims: vec2<u32>,
  num_particles: u32,
  num_types: u32,
  num_cells: u32,
  num_blocks: u32,
  step_index: u32,
  max_candidates: u32,
  interaction_radius: f32,
}

// Komórka siatki cząstki; pozycje poza światem (np. NaN) trafiają do skrajnych komórek.
fn cell_xy(pos: vec2<f32>, u: EngineUniforms) -> vec2<i32> {
  let c = floor((pos + u.world * 0.5) / u.cell_size);
  return clamp(vec2<i32>(c), vec2<i32>(0), vec2<i32>(u.grid_dims) - vec2<i32>(1));
}

fn cell_index(pos: vec2<f32>, u: EngineUniforms) -> u32 {
  let c = cell_xy(pos, u);
  return u32(c.y) * u.grid_dims.x + u32(c.x);
}
