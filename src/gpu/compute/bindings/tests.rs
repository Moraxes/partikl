use super::*;

const WESL: &str = include_str!("../../shaders/engine/bindings.wesl");

/// `(numer, czy uniform, nazwa)` z linii `@group(0) @binding(n) var<…> nazwa: …;` pliku WESL.
fn declared() -> Vec<(u32, bool, String)> {
  WESL
    .lines()
    .filter_map(|line| {
      let rest = line.strip_prefix("@group(0) @binding(")?;
      let (number, rest) = rest.split_once(") var<")?;
      let (address_space, rest) = rest.split_once("> ")?;
      let (name, _) = rest.split_once(':')?;
      Some((number.parse().unwrap(), address_space == "uniform", name.to_string()))
    })
    .collect()
}

#[test]
fn bindings_match_the_wesl_declarations() {
  let rust: Vec<_> = Binding::ALL.iter().map(|b| (*b as u32, b.is_uniform(), b.wesl_name().to_string())).collect();
  assert_eq!(rust, declared());
}

#[test]
fn binding_numbers_are_consecutive() {
  for (i, binding) in Binding::ALL.iter().enumerate() {
    assert_eq!(*binding as usize, i, "{binding:?}");
  }
}
