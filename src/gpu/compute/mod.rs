//! Krok symulacji na GPU jako przebiegi compute w świecie renderowania.
//!
//! Jeden krok to ciąg przebiegów z tabeli [`pipelines::GRID_PASSES`] (budowa siatki:
//! zerowanie, histogram, suma prefiksowa, scatter) i na końcu [`pipelines::FORCES`] (siły
//! i całkowanie, specjalizowane opcjami `SimOptions` przez shader defs).
//!
//! Co klatkę w świecie renderowania:
//! 1. [`buffers::create`] (raz, gdy stan cząstek jest już w świecie renderowania): bufory
//!    siatki i macierz sił,
//! 2. [`pipelines::queue_forces`]: kolejkuje kompilację wariantu sił dla bieżących opcji,
//! 3. [`step::plan`]: ile kroków w tej klatce, uniformy na każdy krok, bind group,
//! 4. [`step::run`] (w `RenderGraph`, przed kamerą): dispatch przebiegów każdego kroku.
//!
//! Czas GPU przebiegów (tylko w benchmarku): [`timing`].

mod bindings;
mod buffers;
mod pipelines;
mod step;
pub mod timing;

use bevy::core_pipeline::schedule::camera_driver;
use bevy::prelude::*;
use bevy::render::render_resource::ShaderType;
use bevy::render::extract_resource::ExtractResourcePlugin;
use bevy::render::renderer::{RenderGraph, RenderGraphSystems};
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};

use super::GpuSim;
use crate::config::*;

/// Rozmiar grupy przebiegów po cząstkach i komórkach; musi zgadzać się z
/// `@workgroup_size` w `grid_build.wesl` i `step_driver.wesl`.
const WORKGROUP: u32 = 64;
/// Komórek na blok sumy prefiksowej; jak `BLOCK` w `prefix_sum.wesl`.
const SCAN_BLOCK: u32 = 256;
/// Próbkowanie sąsiadów powyżej tylu kandydatów (ochrona przed bardzo gęstymi skupiskami).
const MAX_CANDIDATES: u32 = 512;

/// Stałe i geometria kroku (silnik siatki); układ jak `EngineUniforms` w `engine/types.wesl`.
/// Różni się między krokami klatki tylko `step_index`.
#[derive(ShaderType, Clone, Copy, Debug)]
pub struct EngineUniforms {
  pub world: Vec2,
  pub cell_size: Vec2,
  pub grid_dims: UVec2,
  pub num_particles: u32,
  pub num_types: u32,
  pub num_cells: u32,
  pub num_blocks: u32,
  pub step_index: u32,
  pub max_candidates: u32,
  pub interaction_radius: f32,
}

impl EngineUniforms {
  pub fn new(sim: &GpuSim, step_index: u32) -> Self {
    let g = sim.geometry;
    EngineUniforms {
      world: g.world,
      cell_size: g.cell_size,
      grid_dims: g.dims,
      num_particles: sim.num_particles,
      num_types: sim.num_types,
      num_cells: g.num_cells(),
      num_blocks: g.num_cells().div_ceil(SCAN_BLOCK),
      step_index,
      max_candidates: MAX_CANDIDATES,
      interaction_radius: INTERACTION_RADIUS,
    }
  }
}

/// Parametry reguł (promienie, jądro, tarcie…) w osobnym buforze, wspólnym dla wszystkich kroków;
/// układ jak `RuleParams` w `engine/types.wesl`. Silnik ich nie czyta, więc zmiana reguł nie dotyka
/// geometrii.
#[derive(ShaderType, Clone, Copy, Debug)]
pub struct RuleParams {
  pub dt: f32,
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

impl Default for RuleParams {
  fn default() -> Self {
    RuleParams {
      dt: DELTA_TIME as f32,
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
    app.init_resource::<timing::GpuTimings>().add_plugins(ExtractResourcePlugin::<timing::GpuTimings>::default());
    let Some(render_app) = app.get_sub_app_mut(RenderApp) else { return };
    render_app
      .init_resource::<step::Plan>()
      .add_systems(RenderStartup, (pipelines::init, timing::init))
      .add_systems(
        Render,
        (
          buffers::create
            .run_if(resource_exists::<GpuSim>)
            .run_if(not(resource_exists::<buffers::Buffers>))
            .in_set(RenderSystems::PrepareResources),
          pipelines::queue_forces.in_set(RenderSystems::PrepareResources),
          step::plan.in_set(RenderSystems::PrepareBindGroups),
          timing::collect.in_set(RenderSystems::PrepareResources),
        ),
      )
      .add_systems(
        RenderGraph,
        (step::run.before(camera_driver), timing::map.in_set(RenderGraphSystems::Finish)),
      );
  }
}
