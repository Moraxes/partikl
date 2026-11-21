// Rysowanie cząstek wprost z bufora stanu: 6 wierzchołków na cząstkę (quad z elipsą),
// elipsa wydłużona z prędkością, obrót bez trygonometrii. Wierzchołek
// niesie (narożnik quada, numer cząstki), patrz `particle_quads` w `src/gpu/mod.rs`.

import bevy_pbr::render::mesh_view_bindings::view;
import super::types::Particle;

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<storage, read> particles: array<Particle>;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var<storage, read> colors: array<vec4<f32>>;
// x: promień cząstki, y: próg kwadratu prędkości, z: 1 = wygładzona krawędź (alpha blending).
@group(constants::MATERIAL_BIND_GROUP) @binding(2) var<uniform> params: vec4<f32>;

// Obrzeże quada (w promieniach cząstki) na wygładzenie krawędzi: ok. 1 piksela przy promieniu
// 4 pikseli.
const AA_PAD: f32 = 1.25;

struct Vertex {
  // xy: narożnik quada w jednostkach promienia, z: numer cząstki.
  @location(0) position: vec3<f32>,
}

struct VertexOutput {
  @builtin(position) clip: vec4<f32>,
  @location(0) corner: vec2<f32>,
  @location(1) @interpolate(flat) species: u32,
}

// Warstwa cząstki: skrót jej numeru (PCG) jako głębokość. Stan leży w kolejności komórek siatki,
// więc bez tego (opaque, wygrywa ostatnia rysowana) cząstka z późniejszej komórki zawsze
// przykrywałaby wcześniejsze, a granice komórek byłyby widoczne jako prostokątne krawędzie
// plam koloru (szczególnie przy wydłużonych, szybkich cząstkach). Test głębokości nic nie kosztuje
// (500k–1M cząstek: bez różnicy w czasie klatki).
fn layer(index: u32) -> f32 {
  let state = index * 747796405u + 2891336453u;
  let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
  return f32(((word >> 22u) ^ word) & 0xffffu) / 65536.0;
}

@vertex
fn vertex(v: Vertex) -> VertexOutput {
  let p = particles[u32(v.position.z)];
  let corner = v.position.xy;
  let velocity = p.pos - p.last;
  let speed_sq = dot(velocity, velocity);
  var axes = vec2<f32>(params.x);
  var rot = vec2<f32>(1.0, 0.0);
  if speed_sq > params.y {
    let s = log2(2.0 + speed_sq);
    axes = params.x * vec2<f32>(s, 0.75 / s + 0.25);
    // Połowa kąta jako (sin, cos) bez trygonometrii, patrz `rotation_from_velocity`.
    let len = sqrt(speed_sq);
    var hv: vec2<f32>;
    if velocity.x >= 0.0 {
      hv = vec2<f32>(velocity.y, len + velocity.x);
    } else {
      hv = select(-1.0, 1.0, velocity.y >= 0.0) * vec2<f32>(len - velocity.x, velocity.y);
    }
    hv = hv / length(hv);
    rot = vec2<f32>(hv.y * hv.y - hv.x * hv.x, 2.0 * hv.x * hv.y);
  }
  // Z wygładzaniem quad jest większy od okręgu o obrzeże na przejście krycia (poza okręgiem).
  let pad = select(1.0, AA_PAD, params.z > 0.5);
  let local = corner * pad * axes;
  let world = p.pos + vec2<f32>(rot.x * local.x - rot.y * local.y, rot.y * local.x + rot.x * local.y);
  var out: VertexOutput;
  out.clip = view.clip_from_world * vec4<f32>(world, 0.5 * layer(u32(v.position.z)), 1.0);
  out.corner = corner * pad;
  out.species = p.species;
  return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
  let color = colors[in.species].rgb;
  if params.z > 0.5 {
    // Krycie piksela z odległości od krawędzi okręgu (w jednostkach promienia): pełne w
    // środku, liniowe przejście na szerokość piksela. `fwidth` w jednolitym przepływie.
    let d = length(in.corner);
    let w = max(fwidth(d), 1.0e-4);
    let coverage = clamp((1.0 - d) / w + 0.5, 0.0, 1.0);
    return vec4<f32>(color, coverage);
  }
  if dot(in.corner, in.corner) > 1.0 {
    discard;
  }
  return vec4<f32>(color, 1.0);
}
