//! Przebiegi kroku (tabela) i ich pipeline'y compute.

use std::borrow::Cow;
use std::collections::HashMap;

use bevy::prelude::*;
use bevy::render::render_resource::*;
use bevy::shader::ShaderDefVal;

use super::{SCAN_BLOCK, WORKGROUP, bindings};
use crate::gpu::GpuSim;
use crate::options::{Overlap, SimOptions};

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
  /// Warianty `FORCES` wg opcji symulacji, kolejkowane przy pierwszym użyciu.
  pub forces: HashMap<SimOptions, CachedComputePipelineId>,
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

  /// Wszystkie pipeline'y kroku z wariantem sił dla `options` są skompilowane.
  pub fn ready(&self, cache: &PipelineCache, options: &SimOptions) -> bool {
    let Some(&forces) = self.forces.get(options) else { return false };
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

/// Kolejkuje kompilację wariantu sił dla bieżących opcji, jeśli jeszcze go nie ma.
pub fn queue_forces(options: Res<SimOptions>, mut pipelines: ResMut<Pipelines>, cache: Res<PipelineCache>) {
  if !pipelines.forces.contains_key(&*options) {
    let id = pipelines.queue(&cache, &FORCES, forces_defs(&options));
    pipelines.forces.insert(*options, id);
  }
}

/// Opcje jako flagi shadera; patrz nagłówek `rules/default_rules.wesl`.
fn forces_defs(options: &SimOptions) -> Vec<ShaderDefVal> {
  vec![
    ShaderDefVal::Bool("OVERLAP_RESOLVE".into(), options.overlap == Overlap::Resolve),
    ShaderDefVal::Bool("OVERLAP_ADAPTIVE".into(), options.overlap == Overlap::AdaptiveRange),
    ShaderDefVal::Bool("AVERAGE_FORCES".into(), options.average_forces),
  ]
}
