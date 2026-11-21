//! Planowanie kroków w klatce i ich wykonanie (dispatch przebiegów).

use std::sync::atomic::Ordering;

use bevy::prelude::*;
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice, RenderQueue};
use bevy::render::storage::GpuShaderBuffer;

use super::SimUniforms;
use super::bindings;
use super::buffers::Buffers;
use super::pipelines::{FORCES, GRID_PASSES, Pipelines};
use super::timing::{GpuTimings, Timer};
use crate::gpu::{GpuParams, GpuProgress, GpuSim, MAX_STEPS_PER_FRAME};
use crate::options::SimOptions;

/// Kroki tej klatki. Świat główny tylko zwiększa `GpuParams::step_count`; tu wykonujemy
/// różnicę od ostatniej klatki.
#[derive(Resource, Default)]
pub struct Plan {
  /// Kroki już zlecone GPU.
  done: u64,
  /// Kroki do wykonania w tej klatce (0: nic).
  count: u32,
  options: SimOptions,
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
  options: Res<SimOptions>,
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
  plan.options = *options;
  plan.deterministic = params.deterministic;

  // Gdy pipeline się kompiluje (start, hot reload, nowy wariant), kroki czekają.
  if !pipelines.ready(&cache, &plan.options) {
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
  let layout = cache.get_bind_group_layout(&pipelines.layout);
  plan.bind_group = Some(bindings::create_group(&device, &layout, uniforms, &state.buffer, &buffers));
}

/// Dla każdego kroku planu: przebiegi siatki, potem siły. Zwykle jeden przebieg compute na
/// klatkę; przy pomiarze czasu (`GpuTimings`) pierwszy krok klatki ma osobny przebieg ze
/// znacznikami na każdy dispatch.
#[allow(clippy::too_many_arguments)]
pub fn run(
  mut context: RenderContext,
  sim: Option<Res<GpuSim>>,
  plan: Res<Plan>,
  progress: Res<GpuProgress>,
  pipelines: Res<Pipelines>,
  cache: Res<PipelineCache>,
  timings: Option<Res<GpuTimings>>,
  timer: Option<ResMut<Timer>>,
) {
  let Some(sim) = sim else { return };
  let Some(bind_group) = &plan.bind_group else { return };
  // Ponowne sprawdzenie: hot reload może unieważnić pipeline między `plan` a `run`.
  if plan.count == 0 || !pipelines.ready(&cache, &plan.options) {
    return;
  }
  let get = |id| cache.get_compute_pipeline(id).unwrap();
  let forces = pipelines.forces[&plan.options];
  // Dispatche kroku w kolejności: (entry point, pipeline, liczba grup).
  let mut dispatches = Vec::new();
  for (def, &id) in GRID_PASSES.iter().zip(&pipelines.grid) {
    if !def.deterministic_only || plan.deterministic {
      dispatches.push((def.entry, get(id), def.dispatch.groups(&sim)));
    }
  }
  dispatches.push((FORCES.entry, get(forces), FORCES.dispatch.groups(&sim)));

  let timed = timings.is_some_and(|t| t.enabled);
  match timer.filter(|t| timed && t.idle()) {
    None => {
      let encoder = context.command_encoder();
      let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor { label: Some("partikl_step"), ..default() });
      for &offset in &plan.offsets {
        pass.set_bind_group(0, bind_group, &[offset]);
        for &(_, pipeline, groups) in &dispatches {
          pass.set_pipeline(pipeline);
          pass.dispatch_workgroups(groups, 1, 1);
        }
      }
    }
    // Znaczniki tylko w pierwszym kroku klatki: każdy przebieg ze znacznikami kosztuje
    // ok. 20 µs (Metal, M5), więc reszta kroków idzie jednym przebiegiem jak w aplikacji.
    Some(mut timer) => {
      let encoder = context.command_encoder();
      let (&first, rest) = plan.offsets.split_first().unwrap();
      let mut entries = Vec::new();
      for &(entry, pipeline, groups) in &dispatches {
        let writes = timer.writes(entries.len() as u32);
        let mut pass =
          encoder.begin_compute_pass(&ComputePassDescriptor { label: Some(entry), timestamp_writes: Some(writes) });
        pass.set_bind_group(0, bind_group, &[first]);
        pass.set_pipeline(pipeline);
        pass.dispatch_workgroups(groups, 1, 1);
        entries.push(entry);
      }
      if !rest.is_empty() {
        let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor { label: Some("partikl_step"), ..default() });
        for &offset in rest {
          pass.set_bind_group(0, bind_group, &[offset]);
          for &(_, pipeline, groups) in &dispatches {
            pass.set_pipeline(pipeline);
            pass.dispatch_workgroups(groups, 1, 1);
          }
        }
      }
      timer.finish_frame(encoder, 1, entries);
    }
  }
  progress.0.fetch_add(plan.count as u64, Ordering::SeqCst);
}
