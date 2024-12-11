use std::path::PathBuf;
use bevy::prelude::Resource;
use structopt::StructOpt;

#[derive(StructOpt, Debug, Resource)]
#[structopt(name = "partikl")]
pub struct ProgramArgs {
  /// Exit after this many seconds.
  ///
  /// This is useful for profiling/benchmarking.
  #[structopt(long)]
  pub exit_after: Option<f64>,

  /// Seed used for generating the interaction spec.
  ///
  /// By default, the interaction specification is saved to a timestamped file
  /// after being generated (i.e. on every startup), to avoid interesting
  /// simulations being lost. If you wish to prevent this behaviour, use
  /// --no-dump-interaction-spec.
  ///
  /// Note that it's usually more convenient to use the <interaction-spec> arg.
  #[structopt(long)]
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
  #[structopt(parse(from_os_str))]
  pub interaction_spec: Option<PathBuf>,

  #[structopt(short = "t", long, default_value = "3")]
  pub num_types: usize,

  /// Number of particles to be generated initially.
  #[structopt(short = "n", long, default_value = "1000")]
  pub num_particles: usize,

  /// Batch size for parallel operations.
  #[structopt(long, default_value = "20")]
  pub parallel_batch_size: usize,

  /// Prevents the interaction spec from being dumped.
  #[structopt(long)]
  pub no_dump_interaction_spec: bool,
}
