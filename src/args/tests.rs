use clap::Parser;

use super::ProgramArgs;

fn parse(args: &[&str]) -> Result<ProgramArgs, clap::Error> {
  ProgramArgs::try_parse_from(std::iter::once("partikl").chain(args.iter().copied()))
}

#[test]
fn gpu_and_force_averaging_are_on_by_default() {
  let args = parse(&[]).unwrap();
  assert!(!args.cpu);
  assert!(args.sim_options().average_forces);
}

#[test]
fn flags_turn_gpu_and_averaging_off() {
  assert!(parse(&["--cpu"]).unwrap().cpu);
  assert!(!parse(&["--no-average-forces"]).unwrap().sim_options().average_forces);
}

#[test]
fn deterministic_mode_needs_gpu() {
  assert!(parse(&["--gpu-deterministic"]).unwrap().gpu_deterministic);
  assert!(parse(&["--cpu", "--gpu-deterministic"]).is_err());
}
