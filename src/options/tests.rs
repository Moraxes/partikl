use super::*;

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
