use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::render::storage::ShaderBuffer;
use bevy::shader::ShaderRef;

const SHADER: &str = "embedded://partikl/gpu/shaders/engine/render.wesl";

/// Materiał cząstek: vertex shader czyta stan wprost z bufora (bez encji na cząstkę).
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct ParticleMaterial {
  #[storage(0, read_only)]
  pub particles: Handle<ShaderBuffer>,
  #[storage(1, read_only)]
  pub colors: Handle<ShaderBuffer>,
  #[uniform(2)]
  pub params: RenderParams,
  /// Jak `params.z`; wybiera tryb mieszania materiału.
  pub analytic_aa: bool,
}

#[derive(Debug, Clone, Copy, ShaderType)]
pub struct RenderParams {
  pub radius: f32,
  pub velocity_threshold: f32,
  pub analytic_aa: f32,
  // Cofnięcie rysowanej pozycji w sekundach: (1 − α)·DELTA_TIME
  pub lag: f32,
  // DELTA_TIME
  pub tick: f32,
}

impl Material for ParticleMaterial {
  fn vertex_shader() -> ShaderRef {
    SHADER.into()
  }

  fn fragment_shader() -> ShaderRef {
    SHADER.into()
  }

  fn alpha_mode(&self) -> AlphaMode {
    if self.analytic_aa {
      AlphaMode::Blend
    } else {
      AlphaMode::Opaque
    }
  }
}
