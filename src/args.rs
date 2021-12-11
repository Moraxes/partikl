use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(name = "partikl")]
pub struct ProgramArgs {
    #[structopt(long, default_value = "1")]
    pub parallel_batch_size: usize,

    #[structopt(long)]
    pub exit_after: Option<f64>,
}
