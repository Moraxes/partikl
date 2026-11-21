// Typy wspólne silnika siatki i reguł. Układ musi zgadzać się z `src/gpu/mod.rs`.

struct Particle {
  pos: vec2<f32>,
  // Pozycja z poprzedniego kroku (całkowanie Verleta).
  last: vec2<f32>,
  species: u32,
  // Wygładzona liczba sąsiadów (tryb zmiennego zasięgu).
  crowding: f32,
  // Indeks w buforze stanu (stały); w buforze posortowanym wskazuje miejsce zapisu.
  id: u32,
  _pad: u32,
}

struct SimUniforms {
  world: vec2<f32>,
  cell_size: vec2<f32>,
  grid_dims: vec2<u32>,
  num_particles: u32,
  num_types: u32,
  num_cells: u32,
  num_blocks: u32,
  step_index: u32,
  max_candidates: u32,
  dt: f32,
  interaction_radius: f32,
  repulsion_radius: f32,
  repulsion_strength: f32,
  kernel_middle: f32,
  kernel_width: f32,
  average_neighbours: f32,
  adaptive_neighbours: f32,
  adaptive_range_max: f32,
  crowding_smoothing: f32,
  friction: f32,
  velocity_threshold: f32,
}

// Komórka siatki cząstki; pozycje poza światem (np. NaN) trafiają do skrajnych komórek.
fn cell_xy(pos: vec2<f32>, u: SimUniforms) -> vec2<i32> {
  let c = floor((pos + u.world * 0.5) / u.cell_size);
  return clamp(vec2<i32>(c), vec2<i32>(0), vec2<i32>(u.grid_dims) - vec2<i32>(1));
}

fn cell_index(pos: vec2<f32>, u: SimUniforms) -> u32 {
  let c = cell_xy(pos, u);
  return u32(c.y) * u.grid_dims.x + u32(c.x);
}
