use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(name = "partikl")]
pub struct ProgramArgs {
  /// Exit after this many seconds.
  ///
  /// This is useful for profiling/benchmarking.
  #[structopt(long)]
  pub exit_after: Option<f64>,

  /// Number of particles to be generated initially.
  #[structopt(short = "n", long, default_value = "1000")]
  pub number_of_particles: usize,

  /// Batch size for parallel operations.
  #[structopt(long, default_value = "1")]
  pub parallel_batch_size: usize,
}
