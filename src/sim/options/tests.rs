use super::*;

#[test]
fn every_option_set_selects_exactly_one_force_variant() {
  for average_forces in [false, true] {
    for overlap in Overlap::ALL {
      let options = SimOptions { average_forces, overlap };
      let selected = [false, true]
        .into_iter()
        .flat_map(|a| [SPRING, RESOLVE, ADAPTIVE_RANGE].map(move |o| (a, o)))
        .filter(|&(a, o)| options.selects(a, o))
        .count();
      assert_eq!(selected, 1, "{options:?}");
    }
  }
}

#[test]
fn overlap_cycles_through_all_modes() {
  let mut mode = Overlap::default();
  let mut seen = vec![mode];
  for _ in 0..Overlap::ALL.len() {
    mode = mode.next();
    seen.push(mode);
  }
  assert_eq!(seen, [Overlap::Spring, Overlap::Resolve, Overlap::AdaptiveRange, Overlap::Spring]);
}
