//! Bufory siatki i macierz sił; stan cząstek jest osobno (`GpuSim::state`, wspólny z renderem).

use bevy::prelude::*;
use bevy::render::render_resource::*;
use bevy::render::renderer::RenderDevice;

use super::SCAN_BLOCK;
use crate::gpu::{GpuSim, MAX_CELLS};

/// Bufory pomocnicze kroku; opis zawartości w `engine/bindings.wesl`.
#[derive(Resource)]
pub struct Buffers {
  pub sorted_hot: Buffer,
  pub sorted_ids: Buffer,
  pub cell_counts: Buffer,
  pub cell_offsets: Buffer,
  pub cell_cursors: Buffer,
  pub force_matrix: Buffer,
}

pub fn create(mut commands: Commands, sim: Res<GpuSim>, device: Res<RenderDevice>) {
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
    sorted_ids: storage("sorted_ids", n * 4),
    cell_counts: storage("cell_counts", cells * 4),
    // Za komórkami sumy bloków sumy prefiksowej.
    cell_offsets: storage("cell_offsets", (cells + (MAX_CELLS / SCAN_BLOCK) as u64) * 4),
    cell_cursors: storage("cell_cursors", cells * 4),
    force_matrix: device.create_buffer_with_data(&BufferInitDescriptor {
      label: Some("force_matrix"),
      contents: bytemuck::cast_slice(&sim.matrix),
      usage: BufferUsages::STORAGE,
    }),
  });
}
