//! Planowanie kroków w klatce i ich wykonanie (dispatch przebiegów).

use std::sync::atomic::Ordering;

use bevy::prelude::*;
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice, RenderQueue};
use bevy::render::storage::GpuShaderBuffer;

use super::EngineUniforms;
use super::bindings;
use super::buffers::Buffers;
use super::pipelines::{FORCES, ForcesVariant, GRID_PASSES, Pipelines};
use super::timing::{GpuTimings, Timer};
use crate::gpu::{GpuParams, GpuProgress, GpuSim, MAX_STEPS_PER_FRAME, SamplingCap, Substeps};
use crate::gpu::rules::RuleFlags;

/// Podkroki tej klatki. Świat główny tylko zwiększa `GpuParams::step_count` (takty); tu wykonujemy
/// różnicę od ostatniej klatki, każdy takt jako `Substeps` podkroków GPU.
#[derive(Resource)]
pub struct Plan {
  /// Takty już zlecone GPU.
  done: u64,
  /// Podkroki GPU już zlecone (numeruje `step_index`: dekorelacja próbkowania w każdym podkroku).
  gpu_steps: u64,
  /// Podkroki do wykonania w tej klatce (0: nic).
  count: u32,
  /// K, dla którego liczono ostatnio; stan startowy ma prędkości dla `DELTA_TIME`, czyli K = 1.
  substeps: u32,
  /// Iloczyn `dt_nowe / dt_stare` od ostatniego wykonanego kroku; różny od 1 uruchamia `rescale`.
  dt_ratio: f32,
  /// Pierwszy krok tej klatki przeskalowuje prędkości (`When::Rescale`).
  rescale_first: bool,
  variant: ForcesVariant,
  deterministic: bool,
  /// Pierwszy krok tej klatki liczy histogram od zera (`When::Recount`).
  recount: bool,
  /// Czy pierwszy krok planu ma `When::Recount`.
  recount_first: bool,
  /// Bufor stanu i `GpuSim::state_epoch` z ostatniego planu; zmiana (podmiana stanu) wymusza przeliczenie.
  state: Option<(BufferId, u32)>,
  /// Uniformy kolejnych kroków (różnią się `step_index`) i ich przesunięcia w buforze.
  uniforms: DynamicUniformBuffer<EngineUniforms>,
  offsets: Vec<u32>,
  bind_group: Option<BindGroup>,
}

impl Default for Plan {
  fn default() -> Self {
    Plan {
      done: 0,
      gpu_steps: 0,
      count: 0,
      substeps: 1,
      dt_ratio: 1.0,
      rescale_first: false,
      variant: default(),
      deterministic: false,
      recount: false,
      recount_first: false,
      state: None,
      uniforms: default(),
      offsets: default(),
      bind_group: None,
    }
  }
}

#[allow(clippy::too_many_arguments)]
pub fn plan(
  sim: Option<Res<GpuSim>>,
  params: Res<GpuParams>,
  flags: Res<RuleFlags>,
  cap: Res<SamplingCap>,
  substeps: Res<Substeps>,
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
  let (Some(state), Some(model)) = (gpu_buffers.get(&sim.state), gpu_buffers.get(&sim.model)) else { return };
  plan.variant = ForcesVariant::new(&flags, sim.num_types);
  plan.deterministic = params.deterministic;
  // Histogram następnego kroku liczą siły, więc nowe bufory i podmieniony stan (cząstki wskakują w
  // inne komórki bez przebiegu sił) wymagają przeliczenia od zera. Zapis do istniejącego bufora
  // (ten sam rozmiar) nie zmienia jego id, więc podmianę zgłasza `GpuSim::state_epoch`. Flaga czeka na krok, który
  // faktycznie się wykona.
  if buffers.is_changed() || plan.state != Some((state.buffer.id(), sim.state_epoch)) {
    plan.recount = true;
    plan.state = Some((state.buffer.id(), sim.state_epoch));
  }

  // Zmiana K zmienia dt, a Verlet trzyma prędkość jako `pos − last`: skala `dt_nowe / dt_stare`
  // czeka na krok, który faktycznie się wykona (jak `recount`).
  let k = substeps.get();
  if k != plan.substeps {
    plan.dt_ratio *= plan.substeps as f32 / k as f32;
    plan.substeps = k;
  }

  // Gdy pipeline się kompiluje (start, hot reload, nowy wariant), kroki czekają.
  if !pipelines.ready(&cache, &plan.variant) {
    return;
  }
  // Zaległość ponad `MAX_STEPS_PER_FRAME` (zacięcie) pomijamy.
  let pending = params.step_count.saturating_sub(plan.done).min(MAX_STEPS_PER_FRAME as u64);
  plan.done = params.step_count;
  if pending == 0 {
    return;
  }
  let steps = pending as u32 * k;
  plan.count = steps;
  plan.recount_first = std::mem::take(&mut plan.recount);
  let ratio = std::mem::replace(&mut plan.dt_ratio, 1.0);
  plan.rescale_first = ratio != 1.0;

  let first_step = plan.gpu_steps;
  plan.gpu_steps += steps as u64;
  let plan = &mut *plan;
  plan.uniforms.clear();
  plan.offsets.clear();
  for i in 0..steps {
    let mut engine = EngineUniforms::new(&sim, (first_step + i as u64) as u32, cap.0);
    if i == 0 {
      engine.dt_ratio = ratio;
    }
    plan.offsets.push(plan.uniforms.push(&engine));
  }
  plan.uniforms.write_buffer(&device, &queue);
  let Some(uniforms) = plan.uniforms.binding() else { return };
  // Bind group co klatkę: bufor uniformów może zmienić się przy zapisie.
  let layout = cache.get_bind_group_layout(&pipelines.layout);
  plan.bind_group = Some(bindings::create_group(&device, &layout, uniforms, &state.buffer, &model.buffer, &buffers));
}

/// Dla każdego kroku planu: przebiegi siatki, potem siły. Zwykle jeden przebieg compute na
/// klatkę; przy pomiarze czasu (`GpuTimings`) pierwszy krok klatki ma osobny przebieg ze
/// znacznikami na każdy dispatch.
#[allow(clippy::too_many_arguments)]
pub fn run(
  mut context: RenderContext,
  sim: Option<Res<GpuSim>>,
  plan: Res<Plan>,
  buffers: Option<Res<Buffers>>,
  progress: Res<GpuProgress>,
  pipelines: Res<Pipelines>,
  cache: Res<PipelineCache>,
  timings: Option<Res<GpuTimings>>,
  mut timer: Option<ResMut<Timer>>,
) {
  if let Some(timer) = timer.as_mut() {
    timer.resolve(context.command_encoder());
  }
  let (Some(sim), Some(buffers)) = (sim, buffers) else { return };
  let Some(bind_group) = &plan.bind_group else { return };
  // Ponowne sprawdzenie: hot reload może unieważnić pipeline między `plan` a `run`.
  if plan.count == 0 || !pipelines.ready(&cache, &plan.variant) {
    return;
  }
  let get = |id| cache.get_compute_pipeline(id).unwrap();
  let forces = pipelines.forces[&plan.variant];
  // Dispatche kroku w kolejności: (entry point, pipeline, liczba grup).
  let dispatches_for = |recount: bool, rescale: bool| {
    let mut dispatches = Vec::new();
    for (def, &id) in GRID_PASSES.iter().zip(&pipelines.grid) {
      if def.when.runs(plan.deterministic, recount, rescale) {
        dispatches.push((def.entry, get(id), def.dispatch.groups(&sim)));
      }
    }
    dispatches.push((FORCES.entry, get(forces), FORCES.dispatch.groups(&sim)));
    dispatches
  };
  let first_dispatches = dispatches_for(plan.recount_first, plan.rescale_first);
  let rest_dispatches =
    if plan.recount_first || plan.rescale_first { dispatches_for(false, false) } else { first_dispatches.clone() };
  if plan.recount_first {
    // `count` dolicza do `next_counts`, który po podmianie stanu zawiera stary histogram.
    context.command_encoder().clear_buffer(&buffers.next_counts, 0, None);
  }

  let timed = timings.is_some_and(|t| t.enabled);
  match timer.filter(|t| timed && t.idle()) {
    None => {
      let encoder = context.command_encoder();
      let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor { label: Some("partikl_step"), ..default() });
      for (k, &offset) in plan.offsets.iter().enumerate() {
        pass.set_bind_group(0, bind_group, &[offset]);
        for &(_, pipeline, groups) in if k == 0 { &first_dispatches } else { &rest_dispatches } {
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
      for &(entry, pipeline, groups) in &first_dispatches {
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
          for &(_, pipeline, groups) in &rest_dispatches {
            pass.set_pipeline(pipeline);
            pass.dispatch_workgroups(groups, 1, 1);
          }
        }
      }
      timer.finish_frame(1, entries);
    }
  }
  progress.0.fetch_add(plan.count as u64, Ordering::SeqCst);
}
