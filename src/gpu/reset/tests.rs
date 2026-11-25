use std::time::Duration;

use bevy::state::app::StatesPlugin;
use bevy::time::TimeUpdateStrategy;
use clap::Parser;

use super::*;
use crate::loading::load_spec;

const FRAME: Duration = Duration::from_millis(100);

/// Co zmieniono w ostatniej klatce (czyli co poszłoby na GPU).
#[derive(Resource, Default)]
struct Changed {
  spec: bool,
  palette: bool,
}

/// Aplikacja bez GPU z klatkami po 100 ms, z macierzą i paletą (czarną) spoza zakresu losowania.
fn app() -> App {
  let row = Interaction {
    force_coeffs: vec![1000.0; 3],
  };
  let mut spec = ParticleSpec::new(vec![row; 3]);
  spec.params.friction = 0.25;
  let mut app = App::new();
  app
    .add_plugins((MinimalPlugins, StatesPlugin))
    .init_state::<SimState>()
    .insert_resource(spec)
    .insert_resource(TimeUpdateStrategy::ManualDuration(FRAME))
    .insert_resource(Palette(vec![LinearRgba::BLACK; 3]))
    .init_resource::<Changed>()
    .add_plugins(ResetPlugin)
    .add_systems(
      Last,
      |spec: Res<ParticleSpec>, palette: Res<Palette>, mut changed: ResMut<Changed>| {
        changed.spec = spec.is_changed();
        changed.palette = palette.is_changed();
      },
    );
  // Pierwsza klatka ma zerową deltę czasu.
  app.update();
  app
}

fn flat(rows: &[Interaction]) -> Vec<f32> {
  ParticleSpec::new(rows.to_vec()).flat_matrix()
}

fn matrix(app: &App) -> Vec<f32> {
  app.world().resource::<ParticleSpec>().flat_matrix()
}

/// Cel trwającego przejścia.
fn target(app: &App) -> Option<Vec<f32>> {
  let transition = app.world().resource::<MatrixTransition>();
  transition.0.as_ref().map(|tween| flat(&tween.to))
}

fn reroll(app: &mut App) {
  app.world_mut().write_message(RerollMatrix);
  app.update();
}

fn frames(app: &mut App, count: usize) {
  for _ in 0..count {
    app.update();
  }
}

fn set_state(app: &mut App, state: SimState) {
  app
    .world_mut()
    .resource_mut::<NextState<SimState>>()
    .set(state);
}

#[test]
fn matrix_moves_gradually_to_the_target() {
  let mut app = app();
  let from = matrix(&app);
  let params = app.world().resource::<ParticleSpec>().params;
  reroll(&mut app);
  assert_eq!(matrix(&app), from, "klatka naciśnięcia");
  let to = target(&app).expect("przejście");
  assert_ne!(to, from);

  frames(&mut app, 5);
  let midway = matrix(&app);
  assert_ne!(midway, from);
  assert_ne!(midway, to);
  for ((a, b), m) in from.iter().zip(&to).zip(&midway) {
    assert!((a.min(*b)..=a.max(*b)).contains(m), "{a} {m} {b}");
  }
  assert!(app.world().resource::<Changed>().spec);
  assert_eq!(app.world().resource::<ParticleSpec>().params, params);
}

#[test]
fn transition_ends_after_a_second_and_leaves_the_spec_alone() {
  let mut app = app();
  let params = app.world().resource::<ParticleSpec>().params;
  reroll(&mut app);
  let to = target(&app).expect("przejście");
  frames(&mut app, 9);
  assert!(target(&app).is_some(), "przed upływem sekundy");
  assert_ne!(matrix(&app), to);
  app.update();
  assert_eq!(target(&app), None);
  assert_eq!(matrix(&app), to);
  assert_eq!(app.world().resource::<ParticleSpec>().params, params);
  frames(&mut app, 2);
  assert!(!app.world().resource::<Changed>().spec);
}

#[test]
fn pause_freezes_the_transition() {
  let mut app = app();
  reroll(&mut app);
  let to = target(&app).expect("przejście");
  frames(&mut app, 3);
  set_state(&mut app, SimState::Paused);
  app.update();
  let frozen = matrix(&app);
  assert_ne!(frozen, to);
  frames(&mut app, 20);
  assert_eq!(matrix(&app), frozen);
  assert_eq!(target(&app), Some(to.clone()));
  set_state(&mut app, SimState::Running);
  frames(&mut app, 10);
  assert_eq!(matrix(&app), to);
}

#[test]
fn rerolling_midway_starts_from_the_current_matrix() {
  let mut app = app();
  reroll(&mut app);
  let first = target(&app).expect("przejście");
  frames(&mut app, 5);
  reroll(&mut app);
  let current = matrix(&app);
  let transition = app.world().resource::<MatrixTransition>();
  let tween = transition.0.as_ref().expect("przejście");
  assert_eq!(flat(&tween.from), current);
  assert_ne!(flat(&tween.to), first);
  assert_eq!(tween.elapsed, Duration::ZERO);
}

fn palette(app: &App) -> Vec<LinearRgba> {
  app.world().resource::<Palette>().0.clone()
}

/// Cel trwającego przejścia palety.
fn color_target(app: &App) -> Option<Vec<LinearRgba>> {
  let transition = app.world().resource::<ColorTransition>();
  transition.0.as_ref().map(|tween| tween.to.clone())
}

fn reroll_colors(app: &mut App) {
  app.world_mut().write_message(RerollColors);
  app.update();
}

#[test]
fn colors_move_gradually_and_end_after_a_second() {
  let mut app = app();
  let from = palette(&app);
  let spec = matrix(&app);
  reroll_colors(&mut app);
  assert_eq!(palette(&app), from, "klatka naciśnięcia");
  let to = color_target(&app).expect("przejście");
  assert_ne!(to, from);

  frames(&mut app, 5);
  assert_ne!(palette(&app), from);
  assert_ne!(palette(&app), to);
  frames(&mut app, 4);
  assert!(color_target(&app).is_some(), "przed upływem sekundy");
  app.update();
  assert_eq!(color_target(&app), None);
  assert_eq!(palette(&app), to);
  frames(&mut app, 2);
  assert!(!app.world().resource::<Changed>().palette);
  assert_eq!(matrix(&app), spec);
}

#[test]
fn colors_keep_moving_while_paused() {
  let mut app = app();
  set_state(&mut app, SimState::Paused);
  app.update();
  reroll_colors(&mut app);
  let to = color_target(&app).expect("przejście");
  frames(&mut app, 10);
  assert_eq!(palette(&app), to);
}

#[test]
fn rerolling_colors_midway_starts_from_the_current_palette() {
  let mut app = app();
  reroll_colors(&mut app);
  let first = color_target(&app).expect("przejście");
  frames(&mut app, 5);
  reroll_colors(&mut app);
  let current = palette(&app);
  let transition = app.world().resource::<ColorTransition>();
  let tween = transition.0.as_ref().expect("przejście");
  assert_eq!(tween.from, current);
  assert_ne!(tween.to, first);
  assert_eq!(tween.elapsed, Duration::ZERO);
}

#[test]
fn rolled_spec_is_saved_unless_dumping_is_off() {
  let dir = std::env::temp_dir().join(format!("partikl-reset-{}", std::process::id()));
  let _ = std::fs::remove_dir_all(&dir);
  std::fs::create_dir_all(&dir).unwrap();
  let spec = generate_particle_spec(&mut rand::rng(), 3);
  let args = |extra: &[&str]| ProgramArgs::parse_from(std::iter::once(&"partikl").chain(extra));

  assert!(dump_rolled(&dir, None, &spec).unwrap().is_none());
  let off = args(&["--no-dump-interaction-spec"]);
  assert!(dump_rolled(&dir, Some(&off), &spec).unwrap().is_none());
  assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);

  // Plik specyfikacji podany przy starcie nie wyłącza zapisu wylosowanych macierzy.
  for args in [args(&[]), args(&["start.ron"])] {
    let path = dump_rolled(&dir, Some(&args), &spec)
      .unwrap()
      .expect("ścieżka zapisu");
    assert_eq!(load_spec(&path).unwrap().flat_matrix(), spec.flat_matrix());
  }
  std::fs::remove_dir_all(&dir).unwrap();
}
