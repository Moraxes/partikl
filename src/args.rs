use bevy::prelude::Resource;

use crate::sim::{Overlap, SimOptions};
use clap::{ArgAction, Parser};
use std::path::PathBuf;

#[derive(Parser, Debug, Resource)]
#[command(version, name = "partikl")]
pub struct ProgramArgs {
  /// Exit after this many seconds.
  ///
  /// This is useful for profiling/benchmarking.
  #[arg(long)]
  pub exit_after: Option<f64>,

  /// Seed used for generating the interaction spec.
  ///
  /// By default, the interaction specification is saved to a timestamped file
  /// after being generated (i.e. on every startup), to avoid interesting
  /// simulations being lost. If you wish to prevent this behaviour, use
  /// --no-dump-interaction-spec.
  ///
  /// Note that it's usually more convenient to use the <interaction-spec> arg.
  #[arg(long)]
  pub interaction_seed: Option<u64>,

  /// Path to the interaction specification file.
  ///
  /// This file contains the force coefficient matrix and other parameters of
  /// the simulation that are considered "interesting", meaning they influence
  /// large-scale patterns that emerge during the simulation. Particle color,
  /// arrangement, initial velocity, and dish size are not configurable yet.
  ///
  /// If present, --no-dump-interaction-spec is assumed, and --num-types is
  /// ignored.
  #[arg()]
  pub interaction_spec: Option<PathBuf>,

  /// Number of particle types, used when generating the interaction spec.
  #[arg(short = 't', long, default_value_t = 3)]
  pub num_types: usize,

  /// Number of particles to be generated initially.
  #[arg(short = 'n', long, default_value_t = 1000)]
  pub num_particles: usize,

  /// Prevents the interaction spec from being dumped.
  #[arg(long)]
  pub no_dump_interaction_spec: bool,

  /// Radius of each particle.
  #[arg(short = 's', long, default_value_t = 2.0)]
  pub particle_size: f32,

  /// Run the simulation on the CPU instead of the GPU (compute shaders).
  #[arg(long = "cpu", action = ArgAction::SetFalse)]
  pub gpu: bool,

  /// GPU only: sort particles within grid cells so runs are bit-for-bit reproducible.
  #[arg(long, conflicts_with = "gpu")]
  pub gpu_deterministic: bool,

  /// Don't scale each particle's force down when it has more neighbours than usual.
  ///
  /// Averaging is on by default; toggled at runtime with N.
  #[arg(long = "no-average-forces", action = ArgAction::SetFalse)]
  pub average_forces: bool,

  /// How overlapping particles push each other apart.
  ///
  /// Cycled at runtime with O.
  #[arg(long, value_enum, default_value_t = Overlap::Spring)]
  pub overlap: Overlap,
}

impl ProgramArgs {
  /// Opcje symulacji na start.
  pub fn sim_options(&self) -> SimOptions {
    SimOptions { average_forces: self.average_forces, overlap: self.overlap }
  }
}

#[cfg(test)]
mod tests;
