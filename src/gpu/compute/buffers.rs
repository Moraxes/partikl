//! Bufory siatki, macierz sił i parametry reguł; stan cząstek jest osobno (`GpuSim::state`, wspólny z renderem).

use bevy::prelude::*;
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderDevice, RenderQueue};

use super::{RuleParams, SCAN_BLOCK};
use crate::gpu::{GpuParticle, GpuSim, MAX_CELLS};

/// Bufory pomocnicze kroku; opis zawartości w `engine/bindings.wesl`.
#[derive(Resource)]
pub struct Buffers {
  pub sorted_hot: Buffer,
  pub sorted_state: Buffer,
  pub cell_counts: Buffer,
  pub cell_offsets: Buffer,
  pub cell_cursors: Buffer,
  pub force_matrix: Buffer,
  pub rule_params: Buffer,
}

pub fn create(mut commands: Commands, sim: Res<GpuSim>, device: Res<RenderDevice>, queue: Res<RenderQueue>) {
  let n = sim.num_particles as u64;
  let cells = sim.geometry.num_cells() as u64;
  let storage = |label: &str, size: u64| {
    device.create_buffer(&BufferDescriptor {
      label: Some(label),
      size: size.max(4),
      usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
      mapped_at_creation: false,
    })
  };
  commands.insert_resource(Buffers {
    // vec4<f32> na cząstkę
    sorted_hot: storage("sorted_hot", n * 16),
    sorted_state: storage("sorted_state", n * size_of::<GpuParticle>() as u64),
    cell_counts: storage("cell_counts", cells * 4),
    // Za komórkami sumy bloków sumy prefiksowej.
    cell_offsets: storage("cell_offsets", (cells + (MAX_CELLS / SCAN_BLOCK) as u64) * 4),
    cell_cursors: storage("cell_cursors", cells * 4),
    force_matrix: device.create_buffer_with_data(&BufferInitDescriptor {
      label: Some("force_matrix"),
      contents: bytemuck::cast_slice(&sim.matrix),
      usage: BufferUsages::STORAGE,
    }),
    rule_params: {
      let mut contents = UniformBuffer::from(RuleParams::default());
      contents.add_usages(BufferUsages::COPY_DST);
      contents.write_buffer(&device, &queue);
      contents.buffer().expect("just written").clone()
    },
  });
}
