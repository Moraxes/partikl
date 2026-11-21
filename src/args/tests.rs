use clap::Parser;

use super::ProgramArgs;

fn parse(args: &[&str]) -> Result<ProgramArgs, clap::Error> {
  ProgramArgs::try_parse_from(std::iter::once("partikl").chain(args.iter().copied()))
}

#[test]
fn force_averaging_is_on_by_default() {
  assert!(parse(&[]).unwrap().sim_options().average_forces);
  assert!(!parse(&["--no-average-forces"]).unwrap().sim_options().average_forces);
}
