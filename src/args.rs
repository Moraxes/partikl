use bevy::prelude::Resource;
use clap::Parser;
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
  /// large-scale patterns that emerge during the simulation. Definitions of
  /// particle color, arrangement, initial velocity, and dish size or shape can
  /// be specified by the <scene> file.
  ///
  /// If present, --no-dump-interaction spec is assumed, and --num-types is
  /// ignored.
  #[arg()]
  pub interaction_spec: Option<PathBuf>,

  #[arg(short = 't', long, default_value_t = 3)]
  pub num_types: usize,

  /// Number of particles to be generated initially.
  #[arg(short = 'n', long, default_value_t = 1000)]
  pub num_particles: usize,

  /// Prevents the interaction spec from being dumped.
  #[arg(long)]
  pub no_dump_interaction_spec: bool,

  #[arg(short = 's', default_value_t = 2.0)]
  pub particle_size: f32,
}
