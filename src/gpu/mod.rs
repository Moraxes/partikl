//! Symulacja na GPU: stan cząstek w buforach, siatka (counting sort) i siły w compute
//! shaderach (`src/gpu/shaders`), rysowanie wprost z bufora stanu. W testach wyrocznią jest
//! skalarna implementacja f64 (`tests.rs`).

use bevy::asset::{RenderAssetUsages, embedded_asset, load_embedded_asset};
use bevy::camera::{ClearColorConfig, Hdr, RenderTarget, ScalingMode};
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::mesh::PrimitiveTopology;
use bevy::prelude::*;
use bevy::render::extract_resource::{ExtractResource, ExtractResourcePlugin};
use bevy::render::storage::ShaderBuffer;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::render::RenderApp;
use bevy::window::PrimaryWindow;
use bytemuck::{Pod, Zeroable};
use rand::prelude::*;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;

mod compute;
mod step_budget;
pub use compute::RuleParams;
pub use compute::timing::{GpuTimings, PassTimes, entries as timing_entries};
pub use step_budget::{SIM_SPEED, StepBudget, count_step};
use step_budget::StepBudgetPlugin;
pub mod headless;
mod material;
mod rules;
pub use rules::{RuleFlags, RuleSource};

use crate::args::ProgramArgs;
use crate::config::*;
use crate::options::{RuleSchema, SimOptions, SimState};
use crate::screen::ScreenSize;
use crate::spec::ParticleSpec;
use crate::ui::MainCamera;
use material::ParticleMaterial;

/// Największa liczba taktów fizyki na klatkę obrazu (podkroków GPU jest K razy więcej); nadwyżkę (zacięcie) pomijamy. Poniżej
/// tego limitu zakres kroków w klatce wyznacza `StepBudget` (symulacja zwalnia przy przeciążeniu).
pub const MAX_STEPS_PER_FRAME: u32 = 16;

/// Cząstka silnika w buforach GPU; układ jak `Particle` w `engine/types.wesl` (24 B). Pola modelu
/// (np. `crowding`) są w osobnych buforach deklarowanych przez reguły (`GpuSim::model`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct GpuParticle {
  pub pos: [f32; 2],
  /// Pozycja z poprzedniego kroku (całkowanie Verleta).
  pub last: [f32; 2],
  pub species: u32,
  /// Stały identyfikator cząstki; bufor stanu jest w kolejności siatki, nie id.
  pub id: u32,
}

/// Geometria siatki: komórki o boku co najmniej `INTERACTION_RADIUS / GRID_REACH`, więc
/// sąsiedztwo `(2·GRID_REACH + 1)²` komórek pokrywa zasięg oddziaływania. W dużym świecie
/// komórki są większe, żeby było ich najwyżej `MAX_CELLS` (więcej kandydatów na cząstkę,
/// wynik ten sam).
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
    let dims_for = |side: f32| {
      UVec2::new(((world.x / side).floor() as u32).max(MIN_CELLS), ((world.y / side).floor() as u32).max(MIN_CELLS))
    };
    // Najmniejszy bok, przy którym komórek jest nie więcej niż `MAX_CELLS`; pętla tylko dla
    // bardzo wydłużonych światów, w których `MIN_CELLS` podbija krótszą oś.
    let mut side = (INTERACTION_RADIUS / GRID_REACH as f32).max((world.x * world.y / MAX_CELLS as f32).sqrt());
    while dims_for(side).element_product() > MAX_CELLS {
      side *= 1.01;
    }
    let dims = dims_for(side);
    Geometry { world, dims, cell_size: world / dims.as_vec2() }
  }

  pub fn num_cells(&self) -> u32 {
    self.dims.x * self.dims.y
  }
}

/// Stan startowy: pozycje jednostajne w świecie,
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
        id: id as u32,
      }
    })
    .collect()
}

/// Kolory typów cząstek: odcienie rozłożone od losowej fazy o losową rozpiętość.
pub fn random_colors(n: usize, rng: &mut impl Rng) -> Vec<Color> {
  let phase = 360.0 * rng.random::<f32>();
  let spread = 60.0 + 300.0 * rng.random::<f32>();
  (0..n)
    .map(|it| {
      let mut hue = phase + spread * it as f32 / n as f32;
      if hue > 360.0 {
        hue -= 360.0;
      }
      Color::lch(0.5 + 0.5 * rng.random::<f32>(), 0.325 + 0.5 * rng.random::<f32>(), hue)
    })
    .collect()
}

/// Opis symulacji przekazywany do świata renderowania.
#[derive(Resource, Clone, ExtractResource)]
#[extract_app(RenderApp)]
pub struct GpuSim {
  pub state: Handle<ShaderBuffer>,
  /// Model cząstek (pola z reguł) równolegle do `state`: `model_stride` bajtów na cząstkę.
  pub model: Handle<ShaderBuffer>,
  pub model_stride: u32,
  pub num_particles: u32,
  pub num_types: u32,
  pub geometry: Geometry,
  /// Zwiększany przy każdym zapisie do `state` z zewnątrz symulacji (podmiana stanu): krok po
  /// zmianie liczy histogram komórek od zera zamiast brać go z przebiegu sił poprzedniego kroku.
  pub state_epoch: u32,
}

/// Parametry silnika inne niż opcje reguł i licznik kroków: świat renderowania wykonuje
/// różnicę kroków od ostatniej klatki.
#[derive(Resource, Clone, Copy, Debug, Default, ExtractResource)]
#[extract_app(RenderApp)]
pub struct GpuParams {
  /// Kolejność cząstek w komórce po id: trajektorie takie same co do bitu (wolniej).
  pub deterministic: bool,
  /// Takty stałego kroku zlecone silnikowi; GPU wykonuje `step_count × Substeps` podkroków.
  pub step_count: u64,
}

/// Domyślny próg próbkowania sąsiadów (kandydatów na cząstkę); powyżej niego silnik liczy
/// co `every`-tego kandydata z wagą `every`.
pub const DEFAULT_MAX_CANDIDATES: u32 = 512;

/// Wartości klawisza C, w kolejności cyklu.
pub const SAMPLING_CAPS: [u32; 4] = [128, 256, 512, 1024];

/// Próg próbkowania sąsiadów, czytany przy każdym kroku (`EngineUniforms::max_candidates`), więc
/// zmiana (klawisz C, `--max-candidates`, bench) działa od następnego kroku bez przekompilowania.
#[derive(Resource, Clone, Copy, Debug, ExtractResource)]
#[extract_app(RenderApp)]
pub struct SamplingCap(pub u32);

impl Default for SamplingCap {
  fn default() -> Self {
    SamplingCap(DEFAULT_MAX_CANDIDATES)
  }
}

impl SamplingCap {
  /// Następna wartość z `SAMPLING_CAPS`; po największej wraca do najmniejszej, a wartość spoza
  /// listy (z flagi) przechodzi do najbliższej większej.
  pub fn next(self) -> u32 {
    SAMPLING_CAPS.into_iter().find(|&cap| cap > self.0).unwrap_or(SAMPLING_CAPS[0])
  }
}

/// Największa liczba podkroków na takt (`Substeps`).
pub const MAX_SUBSTEPS: u32 = 8;

/// Wartości klawisza S i flagi `--substeps`, w kolejności cyklu.
pub const SUBSTEP_CYCLE: [u32; 3] = [1, 2, 4];

/// Podkroki GPU na takt stałego kroku (`DELTA_TIME`): K podkroków po `dt / K`, więcej pracy GPU
/// na takt za cenę mniejszego kroku czasu (sztywność, stabilność trybu RESOLVE). Zmiana działa od
/// następnego taktu: zapisuje `RuleParams::dt` i przeskalowuje prędkości Verleta (`rescale`).
///
/// `GpuParams::step_count` liczy takty, `GpuProgress` podkroki GPU (takty × K).
#[derive(Resource, Clone, Copy, Debug, ExtractResource)]
#[extract_app(RenderApp)]
pub struct Substeps(pub u32);

impl Default for Substeps {
  fn default() -> Self {
    Substeps(1)
  }
}

impl Substeps {
  /// K ograniczone do `1..=MAX_SUBSTEPS`.
  pub fn get(self) -> u32 {
    self.0.clamp(1, MAX_SUBSTEPS)
  }

  /// Następna wartość z `SUBSTEP_CYCLE`; po największej wraca do najmniejszej, a wartość spoza
  /// cyklu przechodzi do najbliższej większej.
  pub fn next(self) -> u32 {
    SUBSTEP_CYCLE.into_iter().find(|&k| k > self.0).unwrap_or(SUBSTEP_CYCLE[0])
  }
}

/// Liczba podkroków wykonanych na GPU (takty × `Substeps`); `Arc` jest wspólny dla świata głównego i renderowania
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
    // Uchwyty trzymamy: pipeline'y kolejkuje się dopiero po załadowaniu wszystkich modułów.
    let mut library = Vec::new();
    embedded_asset!(app, "shaders/engine/types.wesl");
    library.push(load_embedded_asset!(app, "shaders/engine/types.wesl"));
    embedded_asset!(app, "shaders/engine/uniforms.wesl");
    library.push(load_embedded_asset!(app, "shaders/engine/uniforms.wesl"));
    embedded_asset!(app, "shaders/rules/bindings.wesl");
    library.push(load_embedded_asset!(app, "shaders/rules/bindings.wesl"));
    embedded_asset!(app, "shaders/engine/bindings.wesl");
    library.push(load_embedded_asset!(app, "shaders/engine/bindings.wesl"));
    embedded_asset!(app, "shaders/engine/grid_build.wesl");
    library.push(load_embedded_asset!(app, "shaders/engine/grid_build.wesl"));
    embedded_asset!(app, "shaders/engine/prefix_sum.wesl");
    library.push(load_embedded_asset!(app, "shaders/engine/prefix_sum.wesl"));
    embedded_asset!(app, "shaders/engine/step_driver.wesl");
    library.push(load_embedded_asset!(app, "shaders/engine/step_driver.wesl"));
    embedded_asset!(app, "shaders/engine/render.wesl");
    library.push(load_embedded_asset!(app, "shaders/engine/render.wesl"));
    embedded_asset!(app, "shaders/rules/matrix.wesl");
    library.push(load_embedded_asset!(app, "shaders/rules/matrix.wesl"));
    app.insert_resource(compute::ShaderLibrary(library));
    app
      .insert_resource(Time::<Fixed>::from_seconds(DELTA_TIME))
      .init_state::<SimState>()
      .init_resource::<SimOptions>()
      .init_resource::<GpuParams>()
      .init_resource::<GpuProgress>()
      .init_resource::<SamplingCap>()
      .init_resource::<Substeps>()
      .init_resource::<RenderQuality>()
      .add_plugins((
        MaterialPlugin::<ParticleMaterial>::default(),
        ExtractResourcePlugin::<GpuSim>::default(),
        ExtractResourcePlugin::<ParticleSpec>::default(),
        ExtractResourcePlugin::<rules::RuleFlags>::default(),
        ExtractResourcePlugin::<GpuParams>::default(),
        ExtractResourcePlugin::<GpuProgress>::default(),
        ExtractResourcePlugin::<SamplingCap>::default(),
        ExtractResourcePlugin::<Substeps>::default(),
        rules::RulesPlugin,
        compute::GpuComputePlugin,
      ))
      // Bez okna (testy headless) stan dostarcza wywołujący przez `spawn_sim`.
      .add_systems(Startup, init_gpu.run_if(any_with_component::<PrimaryWindow>))
      .init_resource::<RenderInterpolation>()
      .add_systems(Update, apply_render_quality.run_if(resource_changed::<RenderQuality>))
      .add_systems(Update, update_interpolation.run_if(resource_exists::<GpuSim>))
      .add_plugins(StepBudgetPlugin)
      .add_systems(FixedUpdate, count_step.run_if(in_state(SimState::Running)));
  }
}

/// Czy render interpoluje pozycje między taktami (`params.w` materiału). Wyłączony w testach
/// headless, gdzie zegar stały nie biegnie (ułamek taktu byłby zawsze 0).
#[derive(Resource, Clone, Copy, Debug)]
pub struct RenderInterpolation(pub bool);

impl Default for RenderInterpolation {
  fn default() -> Self {
    RenderInterpolation(true)
  }
}

/// Cofnięcie rysowanej pozycji w podkrokach: `(1 − α)·K`, gdzie `α` to ułamek taktu od ostatniego
/// kroku, a `K` liczba podkroków na takt (`pos − last` to jeden podkrok). Gdy takt przepadł
/// (`dropped`), GPU jest za zegarem i ułamek taktu nie dotyczy jego ostatniego kroku: 0 (rysunek
/// ostatniego stanu bez opóźnienia).
pub fn interpolation_lag(alpha: f32, substeps: u32, dropped: bool) -> f32 {
  if dropped { 0.0 } else { (1.0 - alpha.clamp(0.0, 1.0)) * substeps as f32 }
}

/// Co klatkę przekazuje materiałowi cofnięcie pozycji (`params.w`).
fn update_interpolation(
  enabled: Res<RenderInterpolation>,
  fixed: Res<Time<Fixed>>,
  substeps: Res<Substeps>,
  budget: Res<StepBudget>,
  mut materials: ResMut<Assets<ParticleMaterial>>,
) {
  let lag = if enabled.0 { interpolation_lag(fixed.overstep_fraction(), substeps.get(), budget.dropped()) } else { 0.0 };
  for (_, material) in materials.iter_mut() {
    if material.params.w != lag {
      material.params.w = lag;
    }
  }
}

/// Stosuje `RenderQuality` w trakcie działania (klawisze B i A): format kamery (HDR) i
/// wygładzanie krawędzi w materiale cząstek (tryb mieszania i flaga w shaderze).
fn apply_render_quality(
  quality: Res<RenderQuality>,
  mut commands: Commands,
  cameras: Query<Entity, With<MainCamera>>,
  mut materials: ResMut<Assets<ParticleMaterial>>,
) {
  for camera in &cameras {
    let mut camera = commands.entity(camera);
    if quality.hdr {
      camera.insert(Hdr);
    } else {
      camera.remove::<Hdr>();
    }
    camera.insert(quality.msaa);
    if quality.tonemapping {
      camera.insert((Tonemapping::default(), DebandDither::default()));
    } else {
      camera.insert((Tonemapping::None, DebandDither::Disabled));
    }
  }
  for (_, material) in materials.iter_mut() {
    material.analytic_aa = quality.analytic_aa;
    material.params.z = if quality.analytic_aa { 1.0 } else { 0.0 };
  }
}

#[allow(clippy::too_many_arguments)] // parametry systemu Bevy
fn init_gpu(
  mut commands: Commands,
  args: Res<ProgramArgs>,
  spec: Res<ParticleSpec>,
  schema: Res<RuleSchema>,
  screen: ScreenSize,
  mut params: ResMut<GpuParams>,
  mut cap: ResMut<SamplingCap>,
  mut substeps: ResMut<Substeps>,
  buffers: ResMut<Assets<ShaderBuffer>>,
  meshes: ResMut<Assets<Mesh>>,
  materials: ResMut<Assets<ParticleMaterial>>,
) {
  params.deterministic = args.gpu_deterministic;
  cap.0 = args.max_candidates;
  substeps.0 = args.substeps;
  let mut rng = rand::rng();
  let world = screen.get();
  info!("świat symulacji {world} (ekran)");
  let particles = initial_particles(args.num_particles, spec.interactions.len(), world, &mut rng);
  let quality = RenderQuality { analytic_aa: args.antialias, hdr: args.hdr, ..RenderQuality::default() };
  commands.insert_resource(quality);
  let look = ParticleLook { size: args.particle_size, colors: random_colors(spec.interactions.len(), &mut rng), analytic_aa: quality.analytic_aa };
  spawn_sim(&mut commands, buffers, meshes, materials, particles, &spec, &schema, world, look);

  spawn_camera(&mut commands, None, ScalingMode::WindowSize, &quality);
}

/// Jakość rysowania: wygładzanie krawędzi (MSAA albo analitycznie w fragment shaderze, nie oba
/// naraz), format głównej tekstury kamery (HDR to `Rgba16Float`) i tonemapping z ditheringiem.
///
/// MSAA nie wygładza krawędzi elips: okrąg wycina `discard` w fragment shaderze (jedna decyzja
/// na piksel), więc MSAA kosztuje (ok. 12% renderu przy 500k), a obraz zostaje taki sam. Stąd
/// domyślnie wyłączone. Domyślnie LDR: kamera nie tonemapuje, kolory są żywe (biały 255) i
/// render tańszy (ok. 10%). HDR z tonemappingiem daje kolory przygaszone (biały 196) i wrażenie
/// na ekranach HDR/OLED; w aplikacji przełącza je klawisz B, analityczne AA klawisz A.
#[derive(Resource, Clone, Copy, Debug)]
pub struct RenderQuality {
  pub msaa: Msaa,
  pub hdr: bool,
  pub tonemapping: bool,
  /// Krawędź elipsy wygładzona w fragment shaderze (alpha blending); z MSAA wyłączonym.
  pub analytic_aa: bool,
}

impl Default for RenderQuality {
  fn default() -> Self {
    RenderQuality { msaa: Msaa::Off, hdr: false, tonemapping: true, analytic_aa: false }
  }
}

/// Wygląd cząstek: promień, kolory typów, wygładzenie krawędzi.
pub struct ParticleLook {
  pub size: f32,
  pub colors: Vec<Color>,
  pub analytic_aa: bool,
}

/// Kamera symulacji: rzut ortogonalny na czarnym tle. `target` `None` to okno główne; obraz to
/// rysowanie bez okna (benchmark renderu).
pub fn spawn_camera(commands: &mut Commands, target: Option<Handle<Image>>, scaling_mode: ScalingMode, quality: &RenderQuality) {
  let mut camera = commands.spawn((
    MainCamera { zoom_base: 1.125, zoom_exponent: 1 },
    Camera3d::default(),
    Camera { clear_color: ClearColorConfig::Custom(Color::BLACK), ..Default::default() },
    quality.msaa,
    Transform::from_xyz(0.0, 0.0, 1000.0).looking_at(Vec3::ZERO, Vec3::Y),
    Projection::from(OrthographicProjection { scaling_mode, ..OrthographicProjection::default_3d() }),
  ));
  if quality.hdr {
    camera.insert(Hdr);
  }
  if !quality.tonemapping {
    camera.insert((Tonemapping::None, DebandDither::Disabled));
  }
  if let Some(image) = target {
    camera.insert(RenderTarget::Image(image.into()));
  }
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
  schema: &RuleSchema,
  world: Vec2,
  look: ParticleLook,
) -> Handle<ShaderBuffer> {
  let n = particles.len();
  let colors: Vec<[f32; 4]> = look.colors.into_iter().map(|color| LinearRgba::from(color).to_f32_array()).collect();
  let state = buffers.add(ShaderBuffer::new(particles, RenderAssetUsages::RENDER_WORLD));
  let model_stride = schema.model_stride();
  let model = buffers.add(ShaderBuffer::new(vec![0u32; n * model_stride as usize / 4], RenderAssetUsages::RENDER_WORLD));
  let colors = buffers.add(ShaderBuffer::new(colors, RenderAssetUsages::RENDER_WORLD));

  let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);
  mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, particle_quads(n));
  let material = materials.add(ParticleMaterial {
    particles: state.clone(),
    colors,
    params: Vec4::new(look.size, VELOCITY_THRESHOLD, if look.analytic_aa { 1.0 } else { 0.0 }, 0.0),
    analytic_aa: look.analytic_aa,
  });
  commands.spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(material), NoFrustumCulling));

  commands.insert_resource(GpuSim {
    state: state.clone(),
    model,
    model_stride,
    num_particles: n as u32,
    num_types: spec.interactions.len() as u32,
    geometry: Geometry::new(world),
    state_epoch: 0,
  });
  state
}

/// Narożniki quada (dwa trójkąty) w jednostkach promienia cząstki.
const QUAD: [[f32; 2]; 6] = [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]];

/// Wierzchołki do rysowania: 6 na cząstkę, `(narożnik.x, narożnik.y, numer cząstki)`. Pozycję
/// cząstki czyta vertex shader z bufora stanu. Numer jest w wierzchołku, bo `vertex_index`
/// nie zaczyna się od 0: Bevy pakuje siatki do wspólnego bufora i rysuje od przesunięcia.
fn particle_quads(n: usize) -> Vec<[f32; 3]> {
  // Numer w f32 jest dokładny do 2^24.
  assert!(n <= 1 << 24, "za dużo cząstek do rysowania: {n}");
  (0..n).flat_map(|i| QUAD.map(|[x, y]| [x, y, i as f32])).collect()
}

#[cfg(test)]
mod tests;
