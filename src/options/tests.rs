use super::*;

const HEADER: &str = "
// reguły
//@option average_forces key=N: on=AVERAGE_FORCES, off
//@option overlap key=O: spring, resolve=OVERLAP_RESOLVE, adaptive_range=OVERLAP_ADAPTIVE
fn unrelated() {}
";

fn schema() -> RuleSchema {
  RuleSchema::parse(HEADER).unwrap()
}

#[test]
fn parses_options_keys_choices_and_flags() {
  let schema = schema();
  assert_eq!(schema.options.len(), 2);
  let overlap = schema.option("overlap").unwrap();
  assert_eq!(overlap.key, Some('O'));
  assert_eq!(overlap.keycode(), Some(KeyCode::KeyO));
  assert_eq!(overlap.choices.iter().map(|c| c.label.as_str()).collect::<Vec<_>>(), ["spring", "resolve", "adaptive_range"]);
  assert_eq!(overlap.choices[0].flag, None);
  assert_eq!(overlap.choices[2].flag.as_deref(), Some("OVERLAP_ADAPTIVE"));
  assert_eq!(overlap.title(), "overlap");
}

#[test]
fn defaults_are_first_choices_and_flags_follow_choices() {
  let schema = schema();
  let flags = |options: &SimOptions| schema.flags(options).into_iter().filter(|(_, on)| *on).map(|(flag, _)| flag).collect::<Vec<_>>();
  assert_eq!(flags(&SimOptions::default()), ["AVERAGE_FORCES"]);
  let options = SimOptions::default().with("average_forces", "off").with("overlap", "resolve");
  assert_eq!(flags(&options), ["OVERLAP_RESOLVE"]);
  // Wszystkie zadeklarowane flagi są zawsze podane, żeby shader nie widział niezdefiniowanej.
  assert_eq!(schema.flags(&options).len(), 3);
}

#[test]
fn cycling_visits_every_choice_and_wraps() {
  let schema = schema();
  let overlap = schema.option("overlap").unwrap();
  let mut options = SimOptions::default();
  let seen: Vec<_> = (0..4)
    .map(|_| {
      options.cycle(overlap);
      options.choice(overlap).label.clone()
    })
    .collect();
  assert_eq!(seen, ["resolve", "adaptive_range", "spring", "resolve"]);
}

#[test]
fn combinations_cover_the_product() {
  assert_eq!(schema().combinations().len(), 6);
}

#[test]
fn overrides_are_checked_against_the_schema() {
  let schema = schema();
  let options = SimOptions::from_overrides(&schema, &["overlap=resolve".into(), "average_forces=off".into()]).unwrap();
  assert_eq!(options.value("overlap"), Some("resolve"));
  assert!(SimOptions::from_overrides(&schema, &["overlap=nope".into()]).unwrap_err().contains("spring"));
  assert!(SimOptions::from_overrides(&schema, &["nope=on".into()]).unwrap_err().contains("overlap"));
  assert!(SimOptions::from_overrides(&schema, &["overlap".into()]).is_err());
}

#[test]
fn malformed_declarations_are_errors() {
  for bad in [
    "//@option x: only_one",
    "//@option x key=H: a, b",
    "//@option x key=n: a, b",
    "//@option x: a=F, b=F",
    "//@option x: a, b\n//@option x: c, d",
    "//@option x key=N: a, b\n//@option y key=N: a, b",
    "//@option : a, b",
    "//@option x a, b",
  ] {
    assert!(RuleSchema::parse(bad).is_err(), "{bad}");
  }
}

#[test]
fn source_without_declarations_has_no_options() {
  assert_eq!(RuleSchema::parse("fn f() {}").unwrap(), RuleSchema::default());
}
