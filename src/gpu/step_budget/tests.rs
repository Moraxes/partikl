use super::*;

#[test]
fn long_frames_with_several_steps_shrink_the_limit_to_one_step() {
  let mut limit = MAX_STEPS_PER_FRAME;
  let mut seen = vec![limit];
  for _ in 0..12 {
    limit = next_limit(limit, 0.17, limit, true);
    seen.push(limit);
  }
  assert!(seen.windows(2).all(|w| w[1] <= w[0]), "limit rośnie przy przeciążeniu: {seen:?}");
  assert_eq!(limit, 1, "{seen:?}");
}

#[test]
fn a_single_step_is_never_cut_below_one() {
  // Nawet jeden krok nie mieści się w klatce: limit zostaje 1 (symulacja idzie, wolno).
  assert_eq!(next_limit(1, 0.05, 1, true), 1);
  // Długa klatka bez kroków (np. przeciągnięcie okna) nie jest winą symulacji.
  assert_eq!(next_limit(8, 0.5, 0, false), 8);
}

#[test]
fn limit_recovers_one_step_at_a_time_when_frames_are_short_and_ticks_were_dropped() {
  let mut limit = 1;
  for expected in [2, 3, 4] {
    limit = next_limit(limit, 0.010, limit, true);
    assert_eq!(limit, expected);
  }
  // Bez przepadłych taktów limit nie rośnie (nic go nie ogranicza).
  assert_eq!(next_limit(4, 0.010, 4, false), 4);
  // Klatka blisko docelowej nie zwiększa limitu.
  assert_eq!(next_limit(4, 0.018, 4, true), 4);
  // Limit nie przekracza twardego sufitu.
  assert_eq!(next_limit(MAX_STEPS_PER_FRAME, 0.001, MAX_STEPS_PER_FRAME, true), MAX_STEPS_PER_FRAME);
}

/// Aplikacja z samym zegarem kroków: `count_step` w `FixedUpdate`, wywoływany ręcznie.
fn clock_app(limit: u32) -> App {
  let mut app = App::new();
  app.init_resource::<GpuParams>().insert_resource(StepBudget { limit, ..default() }).add_systems(FixedUpdate, count_step);
  app
}

fn ticks(app: &mut App, n: usize) {
  for _ in 0..n {
    app.world_mut().run_schedule(FixedUpdate);
  }
}

#[test]
fn ticks_over_the_limit_are_dropped() {
  let mut app = clock_app(3);
  ticks(&mut app, 5);
  assert_eq!(app.world().resource::<GpuParams>().step_count, 3);
  let budget = app.world().resource::<StepBudget>();
  assert_eq!((budget.taken, budget.wanted), (3, 5));
}

#[test]
fn simulation_speed_is_the_share_of_ticks_that_ran() {
  let mut app = clock_app(2);
  app.add_systems(Last, end_frame).init_resource::<Time<Real>>();
  app.init_resource::<bevy::diagnostic::DiagnosticsStore>();
  // Klatka 18 ms to strefa, w której regulator nie zmienia limitu (15–20 ms).
  let frame = |app: &mut App| {
    app.world_mut().resource_mut::<Time<Real>>().advance_by(std::time::Duration::from_millis(18));
    app.world_mut().run_schedule(Last);
  };
  // 4 takty na klatkę, 2 kroki: połowa czasu rzeczywistego.
  for _ in 0..200 {
    ticks(&mut app, 4);
    frame(&mut app);
  }
  let speed = app.world().resource::<StepBudget>().speed().unwrap();
  assert!((speed - 0.5).abs() < 0.02, "prędkość {speed}");
  // Pełna prędkość, gdy wszystkie takty się mieszczą.
  app.world_mut().resource_mut::<StepBudget>().limit = MAX_STEPS_PER_FRAME;
  app.world_mut().resource_mut::<StepBudget>().taken_avg = 0.0;
  app.world_mut().resource_mut::<StepBudget>().wanted_avg = 0.0;
  for _ in 0..200 {
    ticks(&mut app, 2);
    frame(&mut app);
  }
  assert!(app.world().resource::<StepBudget>().speed().unwrap() > 0.98);
}
