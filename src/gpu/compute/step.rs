//! Planowanie kroków w klatce i ich wykonanie (dispatch przebiegów).

use std::sync::atomic::Ordering;

use bevy::prelude::*;
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice, RenderQueue};
use bevy::render::storage::GpuShaderBuffer;

use super::SimUniforms;
use super::buffers::Buffers;
use super::pipelines::{FORCES, GRID_PASSES, Pipelines, Variant, variant};
use crate::gpu::{GpuParams, GpuProgress, GpuSim, MAX_STEPS_PER_FRAME};

/// Kroki tej klatki. Świat główny tylko zwiększa `GpuParams::step_count`; tu wykonujemy
/// różnicę od ostatniej klatki.
#[derive(Resource, Default)]
pub struct Plan {
  /// Kroki już zlecone GPU.
  done: u64,
  /// Kroki do wykonania w tej klatce (0: nic).
  count: u32,
  variant: Variant,
  deterministic: bool,
  /// Uniformy kolejnych kroków (różnią się `step_index`) i ich przesunięcia w buforze.
  uniforms: DynamicUniformBuffer<SimUniforms>,
  offsets: Vec<u32>,
  bind_group: Option<BindGroup>,
}

#[allow(clippy::too_many_arguments)]
pub fn plan(
  sim: Option<Res<GpuSim>>,
  params: Res<GpuParams>,
  mut plan: ResMut<Plan>,
  pipelines: Res<Pipelines>,
  buffers: Option<Res<Buffers>>,
  gpu_buffers: Res<RenderAssets<GpuShaderBuffer>>,
  device: Res<RenderDevice>,
  queue: Res<RenderQueue>,
  cache: Res<PipelineCache>,
) {
  plan.count = 0;
  let (Some(sim), Some(buffers)) = (sim, buffers) else { return };
  let Some(state) = gpu_buffers.get(&sim.state) else { return };
  plan.variant = variant(&params);
  plan.deterministic = params.deterministic;

  // Gdy pipeline się kompiluje (start, hot reload, nowy wariant), kroki czekają.
  if !pipelines.ready(&cache, plan.variant) {
    return;
  }
  // Zaległość ponad `MAX_STEPS_PER_FRAME` (zacięcie) pomijamy.
  let pending = params.step_count.saturating_sub(plan.done).min(MAX_STEPS_PER_FRAME as u64);
  plan.done = params.step_count;
  if pending == 0 {
    return;
  }
  plan.count = pending as u32;

  let first_step = params.step_count - pending;
  let plan = &mut *plan;
  plan.uniforms.clear();
  plan.offsets.clear();
  for k in 0..pending {
    plan.offsets.push(plan.uniforms.push(&SimUniforms::new(&sim, (first_step + k) as u32)));
  }
  plan.uniforms.write_buffer(&device, &queue);
  let Some(uniforms) = plan.uniforms.binding() else { return };
  // Bind group co klatkę: bufor uniformów może zmienić się przy zapisie.
  plan.bind_group = Some(device.create_bind_group(
    "partikl_sim",
    &cache.get_bind_group_layout(&pipelines.layout),
    &BindGroupEntries::sequential((
      uniforms,
      state.buffer.as_entire_binding(),
      buffers.sorted_hot.as_entire_binding(),
      buffers.sorted_ids.as_entire_binding(),
      buffers.cell_counts.as_entire_binding(),
      buffers.cell_offsets.as_entire_binding(),
      buffers.cell_cursors.as_entire_binding(),
      buffers.force_matrix.as_entire_binding(),
    )),
  ));
}

/// Dla każdego kroku planu: przebiegi siatki, potem siły.
pub fn run(
  mut context: RenderContext,
  sim: Option<Res<GpuSim>>,
  plan: Res<Plan>,
  progress: Res<GpuProgress>,
  pipelines: Res<Pipelines>,
  cache: Res<PipelineCache>,
) {
  let Some(sim) = sim else { return };
  let Some(bind_group) = &plan.bind_group else { return };
  // Ponowne sprawdzenie: hot reload może unieważnić pipeline między `plan` a `run`.
  if plan.count == 0 || !pipelines.ready(&cache, plan.variant) {
    return;
  }
  let get = |id| cache.get_compute_pipeline(id).unwrap();
  let forces = pipelines.forces[&plan.variant];
  let mut pass = context.command_encoder().begin_compute_pass(&ComputePassDescriptor { label: Some("partikl_step"), ..default() });
  for &offset in &plan.offsets {
    pass.set_bind_group(0, bind_group, &[offset]);
    for (def, &id) in GRID_PASSES.iter().zip(&pipelines.grid) {
      if def.deterministic_only && !plan.deterministic {
        continue;
      }
      pass.set_pipeline(get(id));
      pass.dispatch_workgroups(def.dispatch.groups(&sim), 1, 1);
    }
    pass.set_pipeline(get(forces));
    pass.dispatch_workgroups(FORCES.dispatch.groups(&sim), 1, 1);
  }
  progress.0.fetch_add(plan.count as u64, Ordering::SeqCst);
}
