use clap::Parser;

use super::ProgramArgs;
use crate::options::SimOptions;

fn parse(args: &[&str]) -> Result<ProgramArgs, clap::Error> {
  ProgramArgs::try_parse_from(std::iter::once("partikl").chain(args.iter().copied()))
}

#[test]
fn options_are_repeatable_and_checked_against_the_rules() {
  let args = parse(&[
    "--option",
    "overlap=resolve",
    "--option",
    "average_forces=off",
  ])
  .unwrap();
  let schema = args.rule_source().read().unwrap().schema;
  let options = SimOptions::from_overrides(&schema, &args.options).unwrap();
  assert_eq!(options.value("overlap"), Some("resolve"));
  assert_eq!(options.value("average_forces"), Some("off"));

  let bad = parse(&["--option", "overlap=nope"]).unwrap();
  assert!(SimOptions::from_overrides(&schema, &bad.options).is_err());
}

#[test]
fn substeps_flag_accepts_only_one_two_or_four() {
  assert_eq!(parse(&[]).unwrap().substeps, 1);
  for k in ["1", "2", "4"] {
    assert_eq!(
      parse(&["--substeps", k]).unwrap().substeps,
      k.parse::<u32>().unwrap()
    );
  }
  for bad in ["0", "3", "8", "x"] {
    assert!(
      parse(&["--substeps", bad]).is_err(),
      "{bad} powinno być odrzucone"
    );
  }
}

#[test]
fn no_options_means_the_rules_defaults() {
  assert_eq!(parse(&[]).unwrap().options, Vec::<String>::new());
}

#[test]
fn positional_args_split_into_spec_file_and_overrides() {
  let args = parse(&[
    "spec.ron",
    "params.repulsion_radius=1",
    "interactions.0.force_coeffs.1=-2",
  ])
  .unwrap();
  assert_eq!(args.spec_files(), [&std::path::PathBuf::from("spec.ron")]);
  assert_eq!(
    args.spec_overrides(),
    [
      ("params.repulsion_radius", "1"),
      ("interactions.0.force_coeffs.1", "-2")
    ]
  );
  // Same nadpisania, bez pliku (specyfikacja generowana).
  let only = parse(&["params.friction=0.02"]).unwrap();
  assert!(only.spec_files().is_empty());
  assert_eq!(only.spec_overrides(), [("params.friction", "0.02")]);
  // `=` w ścieżce pliku nie robi z niego nadpisania.
  assert_eq!(parse(&["dir/a=b.ron"]).unwrap().spec_files().len(), 1);
  assert!(parse(&["=1"]).is_err());
}
