//! Przebiegi kroku (tabela) i ich pipeline'y compute.

use std::borrow::Cow;
use std::collections::HashMap;

use bevy::prelude::*;
use bevy::render::render_resource::binding_types::{storage_buffer_sized, uniform_buffer};
use bevy::render::render_resource::*;
use bevy::shader::ShaderDefVal;

use super::{SCAN_BLOCK, SimUniforms, WORKGROUP};
use crate::gpu::{GpuParams, GpuSim};
use crate::sim::{ADAPTIVE_RANGE, RESOLVE};

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
  // sorted_hot, sorted_ids w kolejności komórek (w komórce: kolejność atomicAdd)
  pass("scatter", ShaderFile::Grid, Dispatch::Particles),
  // w komórce: kolejność po id
  Pass { deterministic_only: true, ..pass("sort_cells", ShaderFile::Grid, Dispatch::Cells) },
];

/// Siły i całkowanie: zapisuje nowy stan cząstek. Wariant wg opcji (shader defs).
pub const FORCES: Pass = pass("forces", ShaderFile::Step, Dispatch::Particles);

/// Wariant przebiegu sił: (tryb nakładania, uśrednianie).
pub type Variant = (u8, bool);

#[derive(Resource)]
pub struct Pipelines {
  pub layout: BindGroupLayoutDescriptor,
  /// Pipeline'y `GRID_PASSES` (te same indeksy).
  pub grid: Vec<CachedComputePipelineId>,
  /// Warianty `FORCES`, kolejkowane przy pierwszym użyciu.
  pub forces: HashMap<Variant, CachedComputePipelineId>,
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

  /// Wszystkie pipeline'y kroku z wariantem `variant` są skompilowane.
  pub fn ready(&self, cache: &PipelineCache, variant: Variant) -> bool {
    let Some(&forces) = self.forces.get(&variant) else { return false };
    self.grid.iter().copied().chain([forces]).all(|id| cache.get_compute_pipeline(id).is_some())
  }
}

pub fn init(mut commands: Commands, asset_server: Res<AssetServer>, cache: Res<PipelineCache>) {
  // Wspólny układ wiązań; kolejność jak `@binding` w `engine/bindings.wesl`.
  let layout = BindGroupLayoutDescriptor::new(
    "partikl_sim",
    &BindGroupLayoutEntries::sequential(
      ShaderStages::COMPUTE,
      (
        uniform_buffer::<SimUniforms>(true),
        storage_buffer_sized(false, None),
        storage_buffer_sized(false, None),
        storage_buffer_sized(false, None),
        storage_buffer_sized(false, None),
        storage_buffer_sized(false, None),
        storage_buffer_sized(false, None),
        storage_buffer_sized(false, None),
      ),
    ),
  );
  let shaders = [ShaderFile::Grid, ShaderFile::Scan, ShaderFile::Step]
    .into_iter()
    .map(|file| (file.path(), asset_server.load(file.path())))
    .collect();
  let mut pipelines = Pipelines { layout, grid: Vec::new(), forces: HashMap::new(), shaders };
  pipelines.grid = GRID_PASSES.iter().map(|pass| pipelines.queue(&cache, pass, vec![])).collect();
  commands.insert_resource(pipelines);
}

pub fn variant(params: &GpuParams) -> Variant {
  (params.overlap, params.average)
}

/// Kolejkuje kompilację wariantu sił dla bieżących opcji, jeśli jeszcze go nie ma.
pub fn queue_forces(params: Res<GpuParams>, mut pipelines: ResMut<Pipelines>, cache: Res<PipelineCache>) {
  let variant = variant(&params);
  if !pipelines.forces.contains_key(&variant) {
    let id = pipelines.queue(&cache, &FORCES, forces_defs(variant));
    pipelines.forces.insert(variant, id);
  }
}

fn forces_defs((overlap, average): Variant) -> Vec<ShaderDefVal> {
  vec![
    ShaderDefVal::Bool("OVERLAP_RESOLVE".into(), overlap == RESOLVE),
    ShaderDefVal::Bool("OVERLAP_ADAPTIVE".into(), overlap == ADAPTIVE_RANGE),
    ShaderDefVal::Bool("AVERAGE_FORCES".into(), average),
  ]
}
