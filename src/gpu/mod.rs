//! Symulacja na GPU: stan cząstek w buforach, siatka (counting sort) i siły w compute
//! shaderach (`src/gpu/shaders`), rysowanie wprost z bufora stanu. Silnik CPU (`sim`) zostaje
//! jako punkt odniesienia i wyrocznia w testach. Włączane flagą `--gpu`.

use bevy::asset::RenderAssetUsages;
use bevy::camera::{ClearColorConfig, Hdr, ScalingMode};
use bevy::mesh::PrimitiveTopology;
use bevy::prelude::*;
use bevy::render::extract_resource::{ExtractResource, ExtractResourcePlugin};
use bevy::render::storage::ShaderBuffer;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::render::RenderApp;
use bevy::shader::load_shader_library;
use bevy::window::PrimaryWindow;
use bytemuck::{Pod, Zeroable};
use rand::prelude::*;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;

mod compute;
pub mod headless;
mod material;

use crate::args::ProgramArgs;
use crate::config::*;
use crate::render::{MainCamera, get_random_colors};
use crate::sim::{SimOptions, SimState};
use crate::spec::ParticleSpec;
use material::ParticleMaterial;

/// Największa liczba kroków fizyki na klatkę obrazu; nadwyżkę (zacięcie) pomijamy.
pub const MAX_STEPS_PER_FRAME: u32 = 16;

/// Cząstka w buforach GPU; układ jak `Particle` w `engine/types.wesl`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct GpuParticle {
  pub pos: [f32; 2],
  /// Pozycja z poprzedniego kroku (całkowanie Verleta).
  pub last: [f32; 2],
  pub species: u32,
  pub crowding: f32,
  /// Indeks w buforze stanu.
  pub id: u32,
  pub pad: u32,
}

/// Geometria siatki: komórki o boku co najmniej `INTERACTION_RADIUS / GRID_REACH`, więc
/// sąsiedztwo `(2·GRID_REACH + 1)²` komórek pokrywa zasięg oddziaływania.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Geometry {
  pub world: Vec2,
  pub dims: UVec2,
  pub cell_size: Vec2,
}

/// Komórek na oś co najmniej `2·GRID_REACH + 1`, żeby sąsiednie komórki się nie powtarzały.
const MIN_CELLS: u32 = 2 * GRID_REACH as u32 + 1;
/// Suma prefiksowa obsługuje do 256 bloków po 256 komórek.
pub const MAX_CELLS: u32 = 256 * 256;

impl Geometry {
  pub fn new(world: Vec2) -> Self {
    let side = INTERACTION_RADIUS / GRID_REACH as f32;
    let dims = UVec2::new(
      ((world.x / side).floor() as u32).max(MIN_CELLS),
      ((world.y / side).floor() as u32).max(MIN_CELLS),
    );
    assert!(dims.x * dims.y <= MAX_CELLS, "świat {world} daje za dużo komórek siatki ({dims})");
    Geometry { world, dims, cell_size: world / dims.as_vec2() }
  }

  pub fn num_cells(&self) -> u32 {
    self.dims.x * self.dims.y
  }
}

/// Stan startowy jak na CPU (`render::init_particles`): pozycje jednostajne w świecie,
/// prędkości z ±125, losowe typy.
pub fn initial_particles(n: usize, types: usize, world: Vec2, rng: &mut impl Rng) -> Vec<GpuParticle> {
  let dt = DELTA_TIME as f32;
  (0..n)
    .map(|id| {
      let pos = Vec2::new(rng.random::<f32>() * world.x - world.x / 2.0, rng.random::<f32>() * world.y - world.y / 2.0);
      let velocity = Vec2::new(rng.random::<f32>() * 250.0 - 125.0, rng.random::<f32>() * 250.0 - 125.0);
      GpuParticle {
        pos: pos.to_array(),
        last: (pos - dt * velocity).to_array(),
        species: rng.random_range(0..types) as u32,
        crowding: 0.0,
        id: id as u32,
        pad: 0,
      }
    })
    .collect()
}

/// Macierz sił spłaszczona jako `m[a * T + b]` (siła typu `a` na typ `b`).
pub fn flatten_matrix(spec: &ParticleSpec) -> Vec<f32> {
  spec.interactions.iter().flat_map(|row| row.force_coeffs.iter().copied()).collect()
}

/// Opis symulacji przekazywany do świata renderowania.
#[derive(Resource, Clone, ExtractResource)]
#[extract_app(RenderApp)]
pub struct GpuSim {
  pub state: Handle<ShaderBuffer>,
  pub num_particles: u32,
  pub num_types: u32,
  pub geometry: Geometry,
  pub matrix: Arc<Vec<f32>>,
}

/// Opcje i licznik kroków: świat renderowania wykonuje różnicę kroków od ostatniej klatki.
#[derive(Resource, Clone, Copy, Debug, Default, ExtractResource)]
#[extract_app(RenderApp)]
pub struct GpuParams {
  pub overlap: u8,
  pub average: bool,
  /// Kolejność cząstek w komórce po id: trajektorie takie same co do bitu (wolniej).
  pub deterministic: bool,
  pub step_count: u64,
}

/// Liczba kroków wykonanych na GPU; `Arc` jest wspólny dla świata głównego i renderowania
/// (do wiadomości testów i metryk, bez readbacku).
#[derive(Resource, Clone, Default, ExtractResource)]
#[extract_app(RenderApp)]
pub struct GpuProgress(pub Arc<AtomicU64>);

pub struct GpuPlugin;

impl Plugin for GpuPlugin {
  fn build(&self, app: &mut App) {
    // Shadery są wkompilowane w binarkę (`embedded://partikl/gpu/shaders/...`), więc nie
    // potrzeba katalogu `assets/`. Ładujemy wszystkie moduły, bo importy WESL między
    // modułami wbudowanymi nie doczytują się same. Z feature `dev` zapis pliku przeładowuje
    // shader.
    load_shader_library!(app, "shaders/engine/types.wesl");
    load_shader_library!(app, "shaders/engine/bindings.wesl");
    load_shader_library!(app, "shaders/engine/grid_build.wesl");
    load_shader_library!(app, "shaders/engine/prefix_sum.wesl");
    load_shader_library!(app, "shaders/engine/step_driver.wesl");
    load_shader_library!(app, "shaders/engine/render.wesl");
    load_shader_library!(app, "shaders/rules/default_rules.wesl");
    app
      .insert_resource(Time::<Fixed>::from_seconds(DELTA_TIME))
      .init_state::<SimState>()
      .init_resource::<SimOptions>()
      .init_resource::<GpuParams>()
      .init_resource::<GpuProgress>()
      .add_plugins((
        MaterialPlugin::<ParticleMaterial>::default(),
        ExtractResourcePlugin::<GpuSim>::default(),
        ExtractResourcePlugin::<GpuParams>::default(),
        ExtractResourcePlugin::<GpuProgress>::default(),
        compute::GpuComputePlugin,
      ))
      // Bez okna (testy headless) stan dostarcza wywołujący przez `spawn_sim`.
      .add_systems(Startup, init_gpu.run_if(any_with_component::<PrimaryWindow>))
      .add_systems(Update, sync_options)
      .add_systems(FixedUpdate, count_step.run_if(in_state(SimState::Running)));
  }
}

fn sync_options(options: Res<SimOptions>, mut params: ResMut<GpuParams>) {
  let (overlap, average) = (options.overlap as u8, options.average_forces);
  if params.overlap != overlap || params.average != average {
    params.overlap = overlap;
    params.average = average;
  }
}

fn count_step(mut params: ResMut<GpuParams>) {
  params.step_count += 1;
}

fn init_gpu(
  mut commands: Commands,
  args: Res<ProgramArgs>,
  spec: Res<ParticleSpec>,
  windows: Query<&Window, With<PrimaryWindow>>,
  mut params: ResMut<GpuParams>,
  buffers: ResMut<Assets<ShaderBuffer>>,
  meshes: ResMut<Assets<Mesh>>,
  materials: ResMut<Assets<ParticleMaterial>>,
) {
  params.deterministic = args.gpu_deterministic;
  let mut rng = rand::rng();
  let window = windows.single().expect("no primary window");
  let world = Vec2::new(window.width(), window.height());
  let particles = initial_particles(args.num_particles, spec.interactions.len(), world, &mut rng);
  let colors = get_random_colors(spec.interactions.len(), &mut rng);
  spawn_sim(&mut commands, buffers, meshes, materials, particles, &spec, world, args.particle_size, colors);

  commands.spawn((
    MainCamera { zoom_base: 1.125, zoom_exponent: 1 },
    Camera3d::default(),
    Hdr,
    Camera { clear_color: ClearColorConfig::Custom(Color::BLACK), ..Default::default() },
    Msaa::Sample4,
    Transform::from_xyz(0.0, 0.0, 1000.0).looking_at(Vec3::ZERO, Vec3::Y),
    Projection::from(OrthographicProjection { scaling_mode: ScalingMode::WindowSize, ..OrthographicProjection::default_3d() }),
  ));
}

/// Wysyła `particles` na GPU, tworzy encję do rysowania i zasób `GpuSim`; zwraca uchwyt bufora
/// stanu (do readbacku).
#[allow(clippy::too_many_arguments)]
pub fn spawn_sim(
  commands: &mut Commands,
  mut buffers: ResMut<Assets<ShaderBuffer>>,
  mut meshes: ResMut<Assets<Mesh>>,
  mut materials: ResMut<Assets<ParticleMaterial>>,
  particles: Vec<GpuParticle>,
  spec: &ParticleSpec,
  world: Vec2,
  particle_size: f32,
  colors: Vec<Color>,
) -> Handle<ShaderBuffer> {
  let n = particles.len();
  let colors: Vec<[f32; 4]> = colors.into_iter().map(|color| LinearRgba::from(color).to_f32_array()).collect();
  let state = buffers.add(ShaderBuffer::new(particles, RenderAssetUsages::RENDER_WORLD));
  let colors = buffers.add(ShaderBuffer::new(colors, RenderAssetUsages::RENDER_WORLD));

  // Siatka wierzchołków tylko z licznością (6 na cząstkę); pozycje czyta vertex shader z bufora.
  let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);
  mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0f32; 3]; 6 * n]);
  let material = materials.add(ParticleMaterial {
    particles: state.clone(),
    colors,
    params: Vec4::new(particle_size, VELOCITY_THRESHOLD, 0.0, 0.0),
  });
  commands.spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(material), NoFrustumCulling));

  commands.insert_resource(GpuSim {
    state: state.clone(),
    num_particles: n as u32,
    num_types: spec.interactions.len() as u32,
    geometry: Geometry::new(world),
    matrix: Arc::new(flatten_matrix(spec)),
  });
  state
}

#[cfg(test)]
mod tests;
