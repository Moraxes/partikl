use clap::Parser;

use super::ProgramArgs;
use crate::options::SimOptions;

fn parse(args: &[&str]) -> Result<ProgramArgs, clap::Error> {
  ProgramArgs::try_parse_from(std::iter::once("partikl").chain(args.iter().copied()))
}

#[test]
fn options_are_repeatable_and_checked_against_the_rules() {
  let args = parse(&["--option", "overlap=resolve", "--option", "average_forces=off"]).unwrap();
  let schema = args.rule_source().read().unwrap().schema;
  let options = SimOptions::from_overrides(&schema, &args.options).unwrap();
  assert_eq!(options.value("overlap"), Some("resolve"));
  assert_eq!(options.value("average_forces"), Some("off"));

  let bad = parse(&["--option", "overlap=nope"]).unwrap();
  assert!(SimOptions::from_overrides(&schema, &bad.options).is_err());
}

#[test]
fn no_options_means_the_rules_defaults() {
  assert_eq!(parse(&[]).unwrap().options, Vec::<String>::new());
}
