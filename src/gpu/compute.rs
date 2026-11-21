//! Przebiegi compute kroku symulacji w `RenderGraph`: zerowanie siatki, histogram, suma
//! prefiksowa, scatter i siły z całkowaniem. Pipeline sił jest specjalizowany opcjami
//! (`OVERLAP_MODE`, `AVERAGE_FORCES`) przez shader defs.

use std::borrow::Cow;
use std::collections::HashMap;

use bevy::core_pipeline::schedule::camera_driver;
use bevy::prelude::*;
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_resource::binding_types::{storage_buffer_sized, uniform_buffer};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice, RenderGraph, RenderQueue};
use bevy::render::storage::GpuShaderBuffer;
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};
use bevy::shader::ShaderDefVal;

use super::{GpuParams, GpuProgress, GpuSim, MAX_STEPS_PER_FRAME};
use crate::config::*;
use crate::sim::{ADAPTIVE_RANGE, RESOLVE};

const GRID_SHADER: &str = "embedded://partikl/gpu/shaders/engine/grid_build.wesl";
const SCAN_SHADER: &str = "embedded://partikl/gpu/shaders/engine/prefix_sum.wesl";
const STEP_SHADER: &str = "embedded://partikl/gpu/shaders/engine/step_driver.wesl";
const WORKGROUP: u32 = 64;
/// Rozmiar grupy przebiegu sił; musi zgadzać się z `@workgroup_size` w `step_driver.wesl`.
const FORCES_WORKGROUP: u32 = 64;
const SCAN_BLOCK: u32 = 256;
/// Próbkowanie sąsiadów powyżej tylu kandydatów (jak `NEIGHBOUR_CAP` na CPU).
const MAX_CANDIDATES: u32 = 512;

/// Stałe i geometria kroku; układ jak `SimUniforms` w `engine/types.wesl`.
#[derive(ShaderType, Clone, Copy, Debug)]
pub struct SimUniforms {
  pub world: Vec2,
  pub cell_size: Vec2,
  pub grid_dims: UVec2,
  pub num_particles: u32,
  pub num_types: u32,
  pub num_cells: u32,
  pub num_blocks: u32,
  pub step_index: u32,
  pub max_candidates: u32,
  pub dt: f32,
  pub interaction_radius: f32,
  pub repulsion_radius: f32,
  pub repulsion_strength: f32,
  pub kernel_middle: f32,
  pub kernel_width: f32,
  pub average_neighbours: f32,
  pub adaptive_neighbours: f32,
  pub adaptive_range_max: f32,
  pub crowding_smoothing: f32,
  pub friction: f32,
  pub velocity_threshold: f32,
}

impl SimUniforms {
  pub fn new(sim: &GpuSim, step_index: u32) -> Self {
    let g = sim.geometry;
    SimUniforms {
      world: g.world,
      cell_size: g.cell_size,
      grid_dims: g.dims,
      num_particles: sim.num_particles,
      num_types: sim.num_types,
      num_cells: g.num_cells(),
      num_blocks: g.num_cells().div_ceil(SCAN_BLOCK),
      step_index,
      max_candidates: MAX_CANDIDATES,
      dt: DELTA_TIME as f32,
      interaction_radius: INTERACTION_RADIUS,
      repulsion_radius: REPULSION_RADIUS,
      repulsion_strength: REPULSION_STRENGTH,
      kernel_middle: KERNEL_MIDDLE,
      kernel_width: KERNEL_WIDTH,
      average_neighbours: AVERAGE_NEIGHBOURS,
      adaptive_neighbours: ADAPTIVE_NEIGHBOURS,
      adaptive_range_max: ADAPTIVE_RANGE_MAX,
      crowding_smoothing: CROWDING_SMOOTHING,
      friction: FRICTION_COEFF,
      velocity_threshold: VELOCITY_THRESHOLD,
    }
  }
}

pub struct GpuComputePlugin;

impl Plugin for GpuComputePlugin {
  fn build(&self, app: &mut App) {
    let Some(render_app) = app.get_sub_app_mut(RenderApp) else { return };
    render_app
      .init_resource::<Steps>()
      .add_systems(RenderStartup, init_pipelines)
      .add_systems(Render, prepare.in_set(RenderSystems::PrepareBindGroups))
      .add_systems(RenderGraph, run_steps.before(camera_driver));
  }
}

#[derive(Resource)]
struct Pipelines {
  layout: BindGroupLayoutDescriptor,
  clear: CachedComputePipelineId,
  count: CachedComputePipelineId,
  scan_blocks: CachedComputePipelineId,
  scan_sums: CachedComputePipelineId,
  add_offsets: CachedComputePipelineId,
  scatter: CachedComputePipelineId,
  sort_cells: CachedComputePipelineId,
  step_shader: Handle<Shader>,
  /// Warianty sił wg (tryb nakładania, uśrednianie), kolejkowane przy pierwszym użyciu.
  forces: HashMap<(u8, bool), CachedComputePipelineId>,
}

/// Bufory pomocnicze siatki i uniformy; tworzone, gdy stan cząstek jest już na GPU.
#[derive(Resource)]
struct Buffers {
  sorted_hot: Buffer,
  sorted_ids: Buffer,
  counts: Buffer,
  offsets: Buffer,
  cursors: Buffer,
  matrix: Buffer,
  uniforms: DynamicUniformBuffer<SimUniforms>,
  bind_group: Option<BindGroup>,
  offsets_per_step: Vec<u32>,
}

/// Kroki do wykonania w tej klatce i wariant sił.
#[derive(Resource, Default)]
struct Steps {
  done: u64,
  count: u32,
  variant: (u8, bool),
  deterministic: bool,
}

fn init_pipelines(mut commands: Commands, asset_server: Res<AssetServer>, cache: Res<PipelineCache>) {
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
  let queue = |shader: &Handle<Shader>, entry: &'static str, defs: Vec<ShaderDefVal>| {
    cache.queue_compute_pipeline(ComputePipelineDescriptor {
      label: Some(Cow::Borrowed(entry)),
      layout: vec![layout.clone()],
      shader: shader.clone(),
      shader_defs: defs,
      entry_point: Some(Cow::Borrowed(entry)),
      ..default()
    })
  };
  let grid: Handle<Shader> = asset_server.load(GRID_SHADER);
  let scan: Handle<Shader> = asset_server.load(SCAN_SHADER);
  let step_shader: Handle<Shader> = asset_server.load(STEP_SHADER);
  commands.insert_resource(Pipelines {
    clear: queue(&grid, "clear", vec![]),
    count: queue(&grid, "count", vec![]),
    scatter: queue(&grid, "scatter", vec![]),
    sort_cells: queue(&grid, "sort_cells", vec![]),
    scan_blocks: queue(&scan, "scan_blocks", vec![]),
    scan_sums: queue(&scan, "scan_sums", vec![]),
    add_offsets: queue(&scan, "add_offsets", vec![]),
    layout,
    step_shader,
    forces: HashMap::new(),
  });
}

fn forces_defs(variant: (u8, bool)) -> Vec<ShaderDefVal> {
  vec![
    ShaderDefVal::Bool("OVERLAP_RESOLVE".into(), variant.0 == RESOLVE),
    ShaderDefVal::Bool("OVERLAP_ADAPTIVE".into(), variant.0 == ADAPTIVE_RANGE),
    ShaderDefVal::Bool("AVERAGE_FORCES".into(), variant.1),
  ]
}

#[allow(clippy::too_many_arguments)]
fn prepare(
  mut commands: Commands,
  sim: Option<Res<GpuSim>>,
  params: Res<GpuParams>,
  mut steps: ResMut<Steps>,
  mut pipelines: ResMut<Pipelines>,
  buffers: Option<ResMut<Buffers>>,
  gpu_buffers: Res<RenderAssets<GpuShaderBuffer>>,
  device: Res<RenderDevice>,
  queue: Res<RenderQueue>,
  cache: Res<PipelineCache>,
) {
  let Some(sim) = sim else { return };
  let Some(state) = gpu_buffers.get(&sim.state) else { return };
  let n = sim.num_particles as u64;
  let cells = sim.geometry.num_cells() as u64;
  let Some(mut buffers) = buffers else {
    let create = |label: &str, size: u64| {
      device.create_buffer(&BufferDescriptor {
        label: Some(label),
        size: size.max(4),
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
        mapped_at_creation: false,
      })
    };
    let matrix = device.create_buffer_with_data(&BufferInitDescriptor {
      label: Some("force_matrix"),
      contents: bytemuck::cast_slice(&sim.matrix),
      usage: BufferUsages::STORAGE,
    });
    commands.insert_resource(Buffers {
      sorted_hot: create("sorted_hot", n * 16),
      sorted_ids: create("sorted_ids", n * 4),
      counts: create("cell_counts", cells * 4),
      // Dalej sumy bloków sumy prefiksowej (do 256).
      offsets: create("cell_offsets", (cells + 256) * 4),
      cursors: create("cell_cursors", cells * 4),
      matrix,
      uniforms: DynamicUniformBuffer::default(),
      bind_group: None,
      offsets_per_step: Vec::new(),
    });
    return;
  };

  let variant = (params.overlap, params.average);
  steps.variant = variant;
  steps.deterministic = params.deterministic;
  if !pipelines.forces.contains_key(&variant) {
    let id = cache.queue_compute_pipeline(ComputePipelineDescriptor {
      label: Some(Cow::Borrowed("forces")),
      layout: vec![pipelines.layout.clone()],
      shader: pipelines.step_shader.clone(),
      shader_defs: forces_defs(variant),
      entry_point: Some(Cow::Borrowed("forces")),
      ..default()
    });
    pipelines.forces.insert(variant, id);
  }

  // Kroki od ostatniej klatki. Gdy pipeline się kompiluje (start, hot reload), czekamy;
  // zaległość ponad `MAX_STEPS_PER_FRAME` pomijamy.
  if !pipelines.ready(&cache, variant) {
    steps.count = 0;
    return;
  }
  let pending = params.step_count.saturating_sub(steps.done).min(MAX_STEPS_PER_FRAME as u64);
  steps.done = params.step_count;
  steps.count = pending as u32;

  buffers.uniforms.clear();
  buffers.offsets_per_step.clear();
  let first_step = params.step_count - pending;
  for k in 0..steps.count {
    let offset = buffers.uniforms.push(&SimUniforms::new(&sim, (first_step + k as u64) as u32));
    buffers.offsets_per_step.push(offset);
  }
  if steps.count == 0 {
    return;
  }
  buffers.uniforms.write_buffer(&device, &queue);
  let Some(uniforms) = buffers.uniforms.binding() else { return };
  let bind_group = device.create_bind_group(
    "partikl_sim",
    &cache.get_bind_group_layout(&pipelines.layout),
    &BindGroupEntries::sequential((
      uniforms,
      state.buffer.as_entire_binding(),
      buffers.sorted_hot.as_entire_binding(),
      buffers.sorted_ids.as_entire_binding(),
      buffers.counts.as_entire_binding(),
      buffers.offsets.as_entire_binding(),
      buffers.cursors.as_entire_binding(),
      buffers.matrix.as_entire_binding(),
    )),
  );
  buffers.bind_group = Some(bind_group);
}

impl Pipelines {
  fn ready(&self, cache: &PipelineCache, variant: (u8, bool)) -> bool {
    let Some(forces) = self.forces.get(&variant) else { return false };
    [self.clear, self.count, self.scan_blocks, self.scan_sums, self.add_offsets, self.scatter, self.sort_cells, *forces]
      .into_iter()
      .all(|id| cache.get_compute_pipeline(id).is_some())
  }
}

fn run_steps(
  mut context: RenderContext,
  sim: Option<Res<GpuSim>>,
  steps: Res<Steps>,
  progress: Res<GpuProgress>,
  pipelines: Res<Pipelines>,
  buffers: Option<Res<Buffers>>,
  cache: Res<PipelineCache>,
) {
  let (Some(sim), Some(buffers)) = (sim, buffers) else { return };
  let Some(bind_group) = &buffers.bind_group else { return };
  if steps.count == 0 || !pipelines.ready(&cache, steps.variant) {
    return;
  }
  let get = |id| cache.get_compute_pipeline(id).unwrap();
  let particle_groups = sim.num_particles.div_ceil(WORKGROUP);
  let force_groups = sim.num_particles.div_ceil(FORCES_WORKGROUP);
  let cell_groups = sim.geometry.num_cells().div_ceil(WORKGROUP);
  let block_groups = sim.geometry.num_cells().div_ceil(SCAN_BLOCK);
  let mut pass = context.command_encoder().begin_compute_pass(&ComputePassDescriptor { label: Some("partikl_step"), ..default() });
  for k in 0..steps.count as usize {
    let offset = [buffers.offsets_per_step[k]];
    pass.set_bind_group(0, bind_group, &offset);
    for (pipeline, groups, enabled) in [
      (pipelines.clear, cell_groups, true),
      (pipelines.count, particle_groups, true),
      (pipelines.scan_blocks, block_groups, true),
      (pipelines.scan_sums, 1, true),
      (pipelines.add_offsets, block_groups, true),
      (pipelines.scatter, particle_groups, true),
      (pipelines.sort_cells, cell_groups, steps.deterministic),
      (pipelines.forces[&steps.variant], force_groups, true),
    ] {
      if enabled {
        pass.set_pipeline(get(pipeline));
        pass.dispatch_workgroups(groups, 1, 1);
      }
    }
  }
  progress.0.fetch_add(steps.count as u64, std::sync::atomic::Ordering::SeqCst);
}
