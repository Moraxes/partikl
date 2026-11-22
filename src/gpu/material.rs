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
  /// x: promień cząstki, y: próg kwadratu prędkości dla kształtu, z: 1 = krawędź wygładzona w
  /// fragment shaderze (alpha blending), 0 = ostra krawędź (`discard`), w: cofnięcie pozycji w
  /// podkrokach (`(1 − α)·K`, patrz `interpolation_lag`): rysowana pozycja to `pos − w·(pos − last)`.
  #[uniform(2)]
  pub params: Vec4,
  /// Jak `params.z`; wybiera tryb mieszania materiału.
  pub analytic_aa: bool,
}

impl Material for ParticleMaterial {
  fn vertex_shader() -> ShaderRef {
    SHADER.into()
  }

  fn fragment_shader() -> ShaderRef {
    SHADER.into()
  }

  fn alpha_mode(&self) -> AlphaMode {
    if self.analytic_aa { AlphaMode::Blend } else { AlphaMode::Opaque }
  }
}
