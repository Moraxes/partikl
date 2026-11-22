//! Bufory siatki, macierz sił i parametry reguł; stan cząstek jest osobno (`GpuSim::state`, wspólny z renderem).

use bevy::prelude::*;
use bevy::render::render_resource::encase;
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderDevice, RenderQueue};

use super::stats;
use super::{RuleParams, SCAN_BLOCK};
use crate::gpu::{GpuParticle, GpuSim, MAX_CELLS, Substeps};
use crate::spec::ParticleSpec;

/// Bufory pomocnicze kroku; opis zawartości w `engine/bindings.wesl`.
#[derive(Resource)]
pub struct Buffers {
  pub sorted_hot: Buffer,
  pub sorted_state: Buffer,
  pub sorted_model: Buffer,
  pub cell_counts: Buffer,
  pub cell_offsets: Buffer,
  pub next_counts: Buffer,
  pub cell_ranks: Buffer,
  pub force_matrix: Buffer,
  pub rule_params: Buffer,
  /// Wynik statystyk kroku (`stats.rs`); kopia do odczytu `stats_read` tylko gdy statystyki włączone.
  pub stats: Buffer,
  pub stats_read: Buffer,
}

/// Parametry reguł na GPU: `dt` dzielony przez liczbę podkroków.
fn gpu_params(spec: &ParticleSpec, substeps: Substeps) -> RuleParams {
  RuleParams { dt: spec.params.dt / substeps.get() as f32, ..spec.params }
}

#[allow(clippy::too_many_arguments)]
pub fn create(mut commands: Commands, sim: Res<GpuSim>, spec: Res<ParticleSpec>, substeps: Res<Substeps>, device: Res<RenderDevice>, queue: Res<RenderQueue>) {
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
    sorted_model: storage("sorted_model", n * sim.model_stride as u64),
    cell_counts: storage("cell_counts", cells * 4),
    // Za komórkami sumy bloków sumy prefiksowej.
    cell_offsets: storage("cell_offsets", (cells + (MAX_CELLS / SCAN_BLOCK) as u64) * 4),
    next_counts: storage("next_counts", cells * 4),
    cell_ranks: storage("cell_ranks", n * 4),
    stats: storage("stats", stats::words(sim.num_particles) * 4),
    stats_read: device.create_buffer(&BufferDescriptor {
      label: Some("stats_read"),
      size: stats::words(sim.num_particles) * 4,
      usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
      mapped_at_creation: false,
    }),
    force_matrix: device.create_buffer_with_data(&BufferInitDescriptor {
      label: Some("force_matrix"),
      contents: bytemuck::cast_slice(&spec.flat_matrix()),
      usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
    }),
    rule_params: {
      let mut contents = UniformBuffer::from(gpu_params(&spec, *substeps));
      contents.add_usages(BufferUsages::COPY_DST);
      contents.write_buffer(&device, &queue);
      contents.buffer().expect("just written").clone()
    },
  });
}

/// Zapisuje macierz sił i parametry reguł z `ParticleSpec` do buforów, gdy zmienił się zasób
/// albo `Substeps` (`dt = DELTA_TIME / K`)
/// (bez rekompilacji shaderów). Liczba typów jest stała od startu: spec o innym rozmiarze macierzy
/// jest pomijany.
pub fn upload_spec(
  spec: Res<ParticleSpec>,
  substeps: Res<Substeps>,
  sim: Res<GpuSim>,
  buffers: Res<Buffers>,
  queue: Res<RenderQueue>,
) {
  let matrix = spec.flat_matrix();
  if matrix.len() != (sim.num_types * sim.num_types) as usize {
    error!("spec ma {} współczynników, symulacja {} typów: pomijam zapis", matrix.len(), sim.num_types);
    return;
  }
  queue.write_buffer(&buffers.force_matrix, 0, bytemuck::cast_slice(&matrix));
  let mut bytes = encase::UniformBuffer::new(Vec::new());
  bytes.write(&gpu_params(&spec, *substeps)).expect("RuleParams fits a Vec");
  queue.write_buffer(&buffers.rule_params, 0, bytes.as_ref());
}
