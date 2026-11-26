//! Krok symulacji na GPU jako przebiegi compute w świecie renderowania.
//!
//! Jeden krok to ciąg przebiegów z tabeli [`pipelines::GRID_PASSES`] (budowa siatki:
//! zerowanie, histogram, suma prefiksowa, scatter) i na końcu [`pipelines::FORCES`] (siły
//! i całkowanie, specjalizowane flagami z opcji reguł przez shader defs).
//!
//! Co klatkę w świecie renderowania:
//! 1. [`buffers::create`] (raz, gdy stan cząstek jest już w świecie renderowania): bufory
//!    siatki, macierz sił i parametry reguł; [`buffers::upload_spec`] zapisuje je ponownie, gdy
//!    zmieni się `ParticleSpec`,
//! 2. [`pipelines::queue_forces`]: kolejkuje kompilację wariantu sił dla bieżących opcji,
//! 3. [`step::plan`]: ile kroków w tej klatce, uniformy na każdy krok, bind group,
//! 4. [`step::run`] (w `RenderGraph`, przed kamerą): dispatch przebiegów każdego kroku.
//!
//! Czas GPU przebiegów (tylko w benchmarku): [`timing`]. Statystyki stanu po kroku (opcja, domyślnie
//! wyłączona): [`stats`].

mod bindings;
mod buffers;
mod pipelines;
mod step;
pub use pipelines::ShaderLibrary;
pub mod stats;
pub mod timing;

use bevy::core_pipeline::schedule::camera_driver;
use bevy::prelude::*;
use bevy::render::extract_resource::ExtractResourcePlugin;
use bevy::render::render_resource::ShaderType;
use bevy::render::renderer::{RenderGraph, RenderGraphSystems};
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};
use serde::{Deserialize, Serialize};

use super::{GpuSim, Substeps};
use crate::config::*;
use crate::spec::ParticleSpec;

/// Rozmiar grupy przebiegów po cząstkach i komórkach; musi zgadzać się z
/// `@workgroup_size` w `grid_build.wesl` i `step_driver.wesl`.
const WORKGROUP: u32 = 64;
/// Komórek na blok sumy prefiksowej; jak `BLOCK` w `prefix_sum.wesl`.
const SCAN_BLOCK: u32 = 256;

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
  /// `max_candidates`: próg próbkowania (`SamplingCap`), dzielnik `every` w `step_driver.wesl`.
  pub fn new(sim: &GpuSim, step_index: u32, max_candidates: u32) -> Self {
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
      max_candidates,
      interaction_radius: INTERACTION_RADIUS,
    }
  }
}

/// Parametry reguł (promienie, jądro, tarcie…) w osobnym buforze, wspólnym dla wszystkich kroków;
/// układ jak `RuleParams` w `engine/types.wesl`. Silnik ich nie czyta, więc zmiana reguł nie dotyka
/// geometrii.
///
/// Pliki `.ron` ustawiają je w polu `params` (brakujące pola: wartości z `config.rs`). `dt` i
/// `velocity_threshold` zostają stałe: krok czasowy i próg ruchu dzielą z silnikiem i renderem.
/// Na GPU `dt` to `dt / Substeps` (zapis w `buffers`).
#[derive(ShaderType, Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RuleParams {
  #[serde(skip)]
  pub dt: f32,
  pub repulsion_radius: f32,
  pub kernel_middle: f32,
  pub kernel_width: f32,
  pub attraction_scale: f32,
  pub attraction_sum_ref: f32,
  pub friction: f32,
  #[serde(skip)]
  pub velocity_threshold: f32,
  pub separation_radius: f32,
  pub separation_stiffness: f32,
  pub separation_saturation: f32,
  pub vision_radius: f32,
  pub cohesion_stiffness: f32,
  pub alignment_rate: f32,
  pub target_speed: f32,
  pub drive_rate: f32,
  pub fov_cos: f32,
}

impl RuleParams {
  /// Czy parametry mają sens fizyczny; zasięg oddziaływania nie może przekraczać zasięgu siatki
  /// (`INTERACTION_RADIUS`).
  pub fn validate(&self) -> Result<(), String> {
    let finite = [
      self.repulsion_radius,
      self.kernel_middle,
      self.kernel_width,
      self.attraction_scale,
      self.attraction_sum_ref,
      self.friction,
      self.separation_radius,
      self.separation_stiffness,
      self.separation_saturation,
      self.vision_radius,
      self.cohesion_stiffness,
      self.alignment_rate,
      self.target_speed,
      self.drive_rate,
      self.fov_cos,
    ];
    if finite.iter().any(|v| !v.is_finite()) {
      return Err("parametry reguł muszą być skończone".into());
    }
    if self.kernel_width <= 0.0
      || self.attraction_sum_ref <= 0.0
      || self.separation_saturation <= 0.0
    {
      return Err(
        "kernel_width, attraction_sum_ref i separation_saturation muszą być dodatnie".into(),
      );
    }
    let reach = self.kernel_middle + self.kernel_width;
    if reach > INTERACTION_RADIUS {
      return Err(format!(
        "kernel_middle + kernel_width = {reach} przekracza zasięg siatki {INTERACTION_RADIUS} (config::INTERACTION_RADIUS)"
      ));
    }
    let vision_radius = self.vision_radius;
    if vision_radius > INTERACTION_RADIUS {
      return Err(format!(
        "vision_radius = {vision_radius} przekracza zasięg siatki {INTERACTION_RADIUS} (config::INTERACTION_RADIUS)"
      ));
    }
    let separation_radius = self.separation_radius;
    if separation_radius > INTERACTION_RADIUS {
      return Err(format!(
        "separation_radius = {separation_radius} przekracza zasięg siatki {INTERACTION_RADIUS} (config::INTERACTION_RADIUS)"
      ));
    }
    Ok(())
  }
}

impl Default for RuleParams {
  fn default() -> Self {
    RuleParams {
      dt: DELTA_TIME as f32,
      repulsion_radius: REPULSION_RADIUS,
      kernel_middle: KERNEL_MIDDLE,
      kernel_width: KERNEL_WIDTH,
      attraction_scale: ATTRACTION_SCALE,
      attraction_sum_ref: ATTRACTION_SUM_REF,
      friction: FRICTION_COEFF,
      velocity_threshold: VELOCITY_THRESHOLD,
      separation_radius: SEPARATION_RADIUS,
      separation_stiffness: SEPARATION_STIFFNESS,
      separation_saturation: SEPARATION_SATURATION,
      vision_radius: VISION_RADIUS,
      cohesion_stiffness: COHESION_STIFFNESS,
      alignment_rate: ALIGNMENT_RATE,
      target_speed: TARGET_SPEED,
      drive_rate: DRIVE_RATE,
      fov_cos: FOV_COS,
    }
  }
}

/// Zapis do buforów reguł: zmiana `ParticleSpec` albo `Substeps` (`dt = DELTA_TIME / K`).
fn spec_or_substeps_changed(spec: Res<ParticleSpec>, substeps: Res<Substeps>) -> bool {
  spec.is_changed() || substeps.is_changed()
}

pub struct GpuComputePlugin;

impl Plugin for GpuComputePlugin {
  fn build(&self, app: &mut App) {
    app
      .init_resource::<timing::GpuTimings>()
      .init_resource::<stats::GpuStats>()
      .add_plugins((
        ExtractResourcePlugin::<timing::GpuTimings>::default(),
        ExtractResourcePlugin::<stats::GpuStats>::default(),
        ExtractResourcePlugin::<ShaderLibrary>::default(),
      ));
    let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
      return;
    };
    render_app
      .init_resource::<step::Plan>()
      .init_resource::<stats::Reader>()
      .add_systems(RenderStartup, (pipelines::init, timing::init))
      .add_systems(
        Render,
        (
          buffers::create
            .run_if(resource_exists::<GpuSim>)
            .run_if(not(resource_exists::<buffers::Buffers>))
            .in_set(RenderSystems::PrepareResources),
          buffers::upload_spec
            .run_if(resource_exists::<buffers::Buffers>)
            .run_if(spec_or_substeps_changed)
            .in_set(RenderSystems::PrepareResources)
            .after(buffers::create),
          pipelines::queue_forces.in_set(RenderSystems::PrepareResources),
          step::plan.in_set(RenderSystems::PrepareBindGroups),
          timing::collect.in_set(RenderSystems::PrepareResources),
          stats::collect.in_set(RenderSystems::PrepareResources),
        ),
      )
      .add_systems(
        RenderGraph,
        (
          step::run.before(camera_driver),
          timing::map.in_set(RenderGraphSystems::Finish),
          stats::map.in_set(RenderGraphSystems::Finish),
        ),
      );
  }
}
