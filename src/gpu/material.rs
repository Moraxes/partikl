use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::render::storage::ShaderBuffer;
use bevy::shader::ShaderRef;
use bevy::sprite_render::{AlphaMode2d, Material2d};

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
  /// Jak `params.analytic_aa`; wybiera tryb mieszania materiału.
  pub analytic_aa: bool,
}

/// Parametry rysowania cząstek (`RenderParams` w `render.wesl`).
#[derive(Debug, Clone, Copy, ShaderType)]
pub struct RenderParams {
  /// Promień cząstki.
  pub radius: f32,
  /// Próg `|vel|²` (j²/s²): wolniejsze cząstki są okręgami, szybsze elipsami wzdłuż prędkości.
  pub velocity_threshold: f32,
  /// 1: krawędź wygładzona w fragment shaderze (alpha blending), 0: ostra (`discard`).
  pub analytic_aa: f32,
  /// Cofnięcie rysowanej pozycji w sekundach, `(1 − α)·DELTA_TIME` (patrz `interpolation_lag`).
  pub lag: f32,
  /// Długość taktu (`DELTA_TIME`): `vel·tick` to przemieszczenie na takt, z którego liczony jest
  /// kształt elipsy.
  pub tick: f32,
}

impl Material2d for ParticleMaterial {
  fn vertex_shader() -> ShaderRef {
    SHADER.into()
  }

  fn fragment_shader() -> ShaderRef {
    SHADER.into()
  }

  fn alpha_mode(&self) -> AlphaMode2d {
    if self.analytic_aa {
      AlphaMode2d::Blend
    } else {
      AlphaMode2d::Opaque
    }
  }
}
