use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(name = "partikl")]
pub struct ProgramArgs {
    #[structopt(short = "n", long, default_value = "1000")]
    pub number_of_particles: usize,

    #[structopt(long, default_value = "1")]
    pub parallel_batch_size: usize,

    #[structopt(long)]
    pub exit_after: Option<f64>,
}
