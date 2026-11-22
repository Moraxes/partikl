//! Przebiegi kroku (tabela) i ich pipeline'y compute.

use std::borrow::Cow;
use std::collections::HashMap;

use bevy::prelude::*;
use bevy::render::RenderApp;
use bevy::render::extract_resource::ExtractResource;
use bevy::shader::Shader;
use bevy::render::render_resource::*;
use bevy::shader::ShaderDefVal;

use super::stats::GpuStats;
use super::{SCAN_BLOCK, WORKGROUP, bindings};
use crate::gpu::GpuSim;
use crate::gpu::rules::RuleFlags;

/// Plik shadera z entry pointem przebiegu.
#[derive(Clone, Copy)]
pub enum ShaderFile {
  Grid,
  Scan,
  Step,
  Stats,
}

impl ShaderFile {
  fn path(self) -> &'static str {
    match self {
      ShaderFile::Grid => "embedded://partikl/gpu/shaders/engine/grid_build.wesl",
      ShaderFile::Scan => "embedded://partikl/gpu/shaders/engine/prefix_sum.wesl",
      ShaderFile::Step => "embedded://partikl/gpu/shaders/engine/step_driver.wesl",
      ShaderFile::Stats => "embedded://partikl/gpu/shaders/engine/stats.wesl",
    }
  }
}

/// Liczba grup roboczych przebiegu.
#[derive(Clone, Copy)]
pub enum Dispatch {
  /// Wątek na cząstkę.
  Particles,
  /// Wątek na komórkę siatki.
  Cells,
  /// Grupa na blok sumy prefiksowej (`SCAN_BLOCK` komórek).
  ScanBlocks,
  /// Jedna grupa.
  Once,
}

impl Dispatch {
  pub fn groups(self, sim: &GpuSim) -> u32 {
    let cells = sim.geometry.num_cells();
    match self {
      Dispatch::Particles => sim.num_particles.div_ceil(WORKGROUP),
      Dispatch::Cells => cells.div_ceil(WORKGROUP),
      Dispatch::ScanBlocks => cells.div_ceil(SCAN_BLOCK),
      Dispatch::Once => 1,
    }
  }
}

/// Przebieg compute: entry point `entry` w pliku `file`.
pub struct Pass {
  pub entry: &'static str,
  pub file: ShaderFile,
  pub dispatch: Dispatch,
  /// Kiedy przebieg się wykonuje.
  pub when: When,
}

/// Warunek wykonania przebiegu w kroku.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum When {
  Always,
  /// Tylko w trybie deterministycznym.
  Deterministic,
  /// Tylko w pierwszym kroku po wgraniu stanu (nowe bufory, podmiana stanu): histogram i rangi
  /// liczy wtedy `count`, bo przebieg sił nie miał jeszcze poprzedniego kroku.
  Recount,
  /// Tylko w pierwszym kroku po zmianie `Substeps`: przeskalowanie prędkości Verleta.
  Rescale,
}

impl When {
  pub fn runs(self, deterministic: bool, recount: bool, rescale: bool) -> bool {
    match self {
      When::Always => true,
      When::Deterministic => deterministic,
      When::Recount => recount,
      When::Rescale => rescale,
    }
  }
}

const fn pass(entry: &'static str, file: ShaderFile, dispatch: Dispatch) -> Pass {
  Pass { entry, file, dispatch, when: When::Always }
}

/// Budowa siatki w kolejności wykonania: po niej cząstki są w `sorted_*` w kolejności
/// komórek, a `cell_offsets[c]` wskazuje początek komórki `c`.
pub const GRID_PASSES: [Pass; 8] = [
  // last = pos − (pos − last)·dt_ratio (po zmianie `Substeps`)
  Pass { when: When::Rescale, ..pass("rescale", ShaderFile::Grid, Dispatch::Particles) },
  // next_counts[c] = liczba cząstek w komórce c, cell_ranks[i] = ranga (histogram z `state`)
  Pass { when: When::Recount, ..pass("count", ShaderFile::Grid, Dispatch::Particles) },
  // cell_counts = next_counts, next_counts = 0 (w zwykłym kroku histogram liczą siły poprzedniego kroku)
  pass("clear", ShaderFile::Grid, Dispatch::Cells),
  // cell_offsets = wykluczająca suma prefiksowa cell_counts (trzy przebiegi)
  pass("scan_blocks", ShaderFile::Scan, Dispatch::ScanBlocks),
  pass("scan_sums", ShaderFile::Scan, Dispatch::Once),
  pass("add_offsets", ShaderFile::Scan, Dispatch::ScanBlocks),
  // sorted_hot, sorted_state w kolejności komórek (w komórce: kolejność rang)
  pass("scatter", ShaderFile::Grid, Dispatch::Particles),
  // w komórce: kolejność po id
  Pass { when: When::Deterministic, ..pass("sort_cells", ShaderFile::Grid, Dispatch::Cells) },
];

/// Siły i całkowanie: zapisuje nowy stan cząstek. Wariant wg opcji (shader defs).
pub const FORCES: Pass = pass("forces", ShaderFile::Step, Dispatch::Particles);

/// Statystyki stanu po siłach (opcja, `stats.wesl`): jedna grupa na `WORKGROUP` cząstek.
pub const STATS: Pass = pass("reduce_stats", ShaderFile::Stats, Dispatch::Particles);

/// Moduły shaderów importowane przez inne (ładowane asynchronicznie przez `AssetServer`).
/// Pipeline kolejkowany przed ich załadowaniem kończy się błędem `module not found`.
#[derive(Resource, Clone, ExtractResource)]
#[extract_app(RenderApp)]
pub struct ShaderLibrary(pub Vec<Handle<Shader>>);

impl ShaderLibrary {
  fn loaded(&self, assets: &AssetServer) -> bool {
    self.0.iter().all(|handle| assets.is_loaded_with_dependencies(handle))
  }
}

#[derive(Resource)]
pub struct Pipelines {
  pub layout: BindGroupLayoutDescriptor,
  /// Pipeline'y `GRID_PASSES` (te same indeksy); puste, dopóki moduły shaderów się ładują.
  pub grid: Vec<CachedComputePipelineId>,
  /// Warianty `FORCES` wg opcji symulacji i liczby typów, kolejkowane przy pierwszym użyciu.
  pub forces: HashMap<ForcesVariant, CachedComputePipelineId>,
  /// Pipeline `STATS`: kolejkowany dopiero, gdy statystyki są włączone po raz pierwszy.
  pub stats: Option<CachedComputePipelineId>,
  shaders: HashMap<&'static str, Handle<Shader>>,
}

impl Pipelines {
  fn queue(&self, cache: &PipelineCache, pass: &Pass, shader_defs: Vec<ShaderDefVal>) -> CachedComputePipelineId {
    cache.queue_compute_pipeline(ComputePipelineDescriptor {
      label: Some(Cow::Borrowed(pass.entry)),
      layout: vec![self.layout.clone()],
      shader: self.shaders[pass.file.path()].clone(),
      shader_defs,
      entry_point: Some(Cow::Borrowed(pass.entry)),
      ..default()
    })
  }

  /// Wszystkie pipeline'y kroku z wariantem sił `variant` są skompilowane.
  pub fn ready(&self, cache: &PipelineCache, variant: &ForcesVariant) -> bool {
    let Some(&forces) = self.forces.get(variant) else { return false };
    !self.grid.is_empty() && self.grid.iter().copied().chain([forces]).all(|id| cache.get_compute_pipeline(id).is_some())
  }
}

pub fn init(mut commands: Commands, asset_server: Res<AssetServer>) {
  let shaders = [ShaderFile::Grid, ShaderFile::Scan, ShaderFile::Step, ShaderFile::Stats]
    .into_iter()
    .map(|file| (file.path(), asset_server.load(file.path())))
    .collect();
  let pipelines =
    Pipelines { layout: bindings::layout(), grid: Vec::new(), forces: HashMap::new(), stats: None, shaders };
  commands.insert_resource(pipelines);
}

/// Jak shader sił czyta współczynniki macierzy sił: dla małej liczby typów kolumna macierzy dla
/// własnego typu cząstki leży w rejestrach (`vec4`), bo odczyt macierzy w pętli po sąsiadach
/// kosztował ok. 20% przebiegu sił (zależny odczyt: sąsiad → typ → macierz).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TypeClass {
  /// Do 4 typów: jeden `vec4`.
  #[default]
  AtMost4,
  /// Do 8 typów: dwa `vec4` i `select`.
  AtMost8,
  /// Więcej: odczyt macierzy z bufora.
  Many,
}

impl TypeClass {
  pub fn of(num_types: u32) -> TypeClass {
    match num_types {
      0..=4 => TypeClass::AtMost4,
      5..=8 => TypeClass::AtMost8,
      _ => TypeClass::Many,
    }
  }
}

/// Wariant pipeline'u sił: flagi zadeklarowane przez reguły (`gpu::rules`) i klasa liczby typów
/// (flagi silnika dla `rules/matrix.wesl`). Kod reguł to osobny moduł, podmieniany bez nowego wariantu.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct ForcesVariant {
  pub flags: Vec<(String, bool)>,
  pub types: TypeClass,
}

impl ForcesVariant {
  pub fn new(flags: &RuleFlags, num_types: u32) -> Self {
    ForcesVariant { flags: flags.0.clone(), types: TypeClass::of(num_types) }
  }
}

/// Kolejkuje pipeline'y siatki (raz) i kompilację wariantu sił dla bieżących opcji i liczby
/// typów, jeśli jeszcze go nie ma. Nic nie robi, dopóki nie załadują się moduły shaderów.
pub fn queue_forces(
  library: Res<ShaderLibrary>,
  assets: Res<AssetServer>,
  flags: Res<RuleFlags>,
  sim: Option<Res<GpuSim>>,
  gpu_stats: Res<GpuStats>,
  mut pipelines: ResMut<Pipelines>,
  cache: Res<PipelineCache>,
) {
  if !library.loaded(&assets) {
    return;
  }
  if pipelines.grid.is_empty() {
    let grid = GRID_PASSES.iter().map(|pass| pipelines.queue(&cache, pass, vec![])).collect();
    pipelines.grid = grid;
  }
  if gpu_stats.enabled && pipelines.stats.is_none() {
    let id = pipelines.queue(&cache, &STATS, vec![]);
    pipelines.stats = Some(id);
  }
  let Some(sim) = sim else { return };
  let variant = ForcesVariant::new(&flags, sim.num_types);
  if !pipelines.forces.contains_key(&variant) {
    let id = pipelines.queue(&cache, &FORCES, forces_defs(&variant));
    pipelines.forces.insert(variant, id);
  }
}

/// Wariant jako flagi shadera: te z reguł (`//@option` w ich nagłówku) i dwie silnika (klasa typów).
fn forces_defs(variant: &ForcesVariant) -> Vec<ShaderDefVal> {
  variant
    .flags
    .iter()
    .map(|(flag, on)| ShaderDefVal::Bool(flag.clone().into(), *on))
    .chain([
      ShaderDefVal::Bool("TYPES_AT_MOST_4".into(), variant.types == TypeClass::AtMost4),
      ShaderDefVal::Bool("TYPES_AT_MOST_8".into(), variant.types == TypeClass::AtMost8),
    ])
    .collect()
}
