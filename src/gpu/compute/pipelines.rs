//! Przebiegi kroku (tabela) i ich pipeline'y compute.

use std::borrow::Cow;
use std::collections::HashMap;

use bevy::prelude::*;
use bevy::render::render_resource::*;
use bevy::shader::ShaderDefVal;

use super::{SCAN_BLOCK, WORKGROUP, bindings};
use crate::gpu::GpuSim;
use crate::gpu::rules::RuleFlags;

/// Plik shadera z entry pointem przebiegu.
#[derive(Clone, Copy)]
pub enum ShaderFile {
  Grid,
  Scan,
  Step,
}

impl ShaderFile {
  fn path(self) -> &'static str {
    match self {
      ShaderFile::Grid => "embedded://partikl/gpu/shaders/engine/grid_build.wesl",
      ShaderFile::Scan => "embedded://partikl/gpu/shaders/engine/prefix_sum.wesl",
      ShaderFile::Step => "embedded://partikl/gpu/shaders/engine/step_driver.wesl",
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
  /// Tylko w trybie deterministycznym.
  pub deterministic_only: bool,
}

const fn pass(entry: &'static str, file: ShaderFile, dispatch: Dispatch) -> Pass {
  Pass { entry, file, dispatch, deterministic_only: false }
}

/// Budowa siatki w kolejności wykonania: po niej cząstki są w `sorted_*` w kolejności
/// komórek, a `cell_offsets[c]` wskazuje początek komórki `c`.
pub const GRID_PASSES: [Pass; 7] = [
  // cell_counts = 0
  pass("clear", ShaderFile::Grid, Dispatch::Cells),
  // cell_counts[c] = liczba cząstek w komórce c
  pass("count", ShaderFile::Grid, Dispatch::Particles),
  // cell_offsets = wykluczająca suma prefiksowa cell_counts (trzy przebiegi)
  pass("scan_blocks", ShaderFile::Scan, Dispatch::ScanBlocks),
  pass("scan_sums", ShaderFile::Scan, Dispatch::Once),
  pass("add_offsets", ShaderFile::Scan, Dispatch::ScanBlocks),
  // sorted_hot, sorted_state w kolejności komórek (w komórce: kolejność atomicAdd)
  pass("scatter", ShaderFile::Grid, Dispatch::Particles),
  // w komórce: kolejność po id
  Pass { deterministic_only: true, ..pass("sort_cells", ShaderFile::Grid, Dispatch::Cells) },
];

/// Siły i całkowanie: zapisuje nowy stan cząstek. Wariant wg opcji (shader defs).
pub const FORCES: Pass = pass("forces", ShaderFile::Step, Dispatch::Particles);

#[derive(Resource)]
pub struct Pipelines {
  pub layout: BindGroupLayoutDescriptor,
  /// Pipeline'y `GRID_PASSES` (te same indeksy).
  pub grid: Vec<CachedComputePipelineId>,
  /// Warianty `FORCES` wg opcji symulacji i liczby typów, kolejkowane przy pierwszym użyciu.
  pub forces: HashMap<ForcesVariant, CachedComputePipelineId>,
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
    self.grid.iter().copied().chain([forces]).all(|id| cache.get_compute_pipeline(id).is_some())
  }
}

pub fn init(mut commands: Commands, asset_server: Res<AssetServer>, cache: Res<PipelineCache>) {
  let shaders = [ShaderFile::Grid, ShaderFile::Scan, ShaderFile::Step]
    .into_iter()
    .map(|file| (file.path(), asset_server.load(file.path())))
    .collect();
  let mut pipelines = Pipelines { layout: bindings::layout(), grid: Vec::new(), forces: HashMap::new(), shaders };
  pipelines.grid = GRID_PASSES.iter().map(|pass| pipelines.queue(&cache, pass, vec![])).collect();
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

/// Kolejkuje kompilację wariantu sił dla bieżących opcji i liczby typów, jeśli jeszcze go nie ma.
pub fn queue_forces(
  flags: Res<RuleFlags>,
  sim: Option<Res<GpuSim>>,
  mut pipelines: ResMut<Pipelines>,
  cache: Res<PipelineCache>,
) {
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
