use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::AsBindGroup;
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
  /// x: promień cząstki, y: próg kwadratu prędkości dla kształtu.
  #[uniform(2)]
  pub params: Vec4,
}

impl Material for ParticleMaterial {
  fn vertex_shader() -> ShaderRef {
    SHADER.into()
  }

  fn fragment_shader() -> ShaderRef {
    SHADER.into()
  }
}
