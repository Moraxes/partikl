use super::*;
use crate::sim::Overlap;
use crate::ui::test_util::*;
use bevy::camera::{OrthographicProjection, Projection};
use bevy::input::mouse::MouseScrollUnit;
use bevy::input::touch::TouchPhase;
use bevy::window::WindowMode;

fn tap_key(app: &mut App, key: KeyCode) {
  app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(key);
  app.update();
  clear_input(app);
  app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(key);
  clear_input(app);
  // Zmiana `State` zostaje zastosowana w następnej klatce.
  app.update();
}

fn primary_window_mode(app: &mut App) -> WindowMode {
  app
    .world_mut()
    .query_filtered::<&Window, With<PrimaryWindow>>()
    .single(app.world())
    .unwrap()
    .mode
}

fn spawn_camera(app: &mut App, translation: Vec3, scale: f32) -> Entity {
  app
    .world_mut()
    .spawn((
      MainCamera { zoom_base: 1.125, zoom_exponent: 1 },
      Transform::from_translation(translation),
      Projection::from(OrthographicProjection { scale, ..OrthographicProjection::default_3d() }),
    ))
    .id()
}

fn camera_scale(app: &App, camera: Entity) -> f32 {
  match app.world().get::<Projection>(camera).unwrap() {
    Projection::Orthographic(p) => p.scale,
    _ => panic!("kamera nie jest ortograficzna"),
  }
}

// --- handle_keyboard_input ---

#[test]
fn space_toggles_pause() {
  let mut app = input_app();
  app.add_systems(Update, handle_keyboard_input);
  app.update();
  assert_eq!(*app.world().resource::<State<SimState>>().get(), SimState::Running);

  tap_key(&mut app, KeyCode::Space);
  assert_eq!(*app.world().resource::<State<SimState>>().get(), SimState::Paused);

  tap_key(&mut app, KeyCode::Space);
  assert_eq!(*app.world().resource::<State<SimState>>().get(), SimState::Running);
}

#[test]
fn unrelated_key_does_not_change_sim_state() {
  let mut app = input_app();
  app.add_systems(Update, handle_keyboard_input);
  tap_key(&mut app, KeyCode::KeyA);
  assert_eq!(*app.world().resource::<State<SimState>>().get(), SimState::Running);
}

#[test]
fn n_toggles_force_averaging() {
  let mut app = input_app();
  app.add_systems(Update, handle_keyboard_input);
  tap_key(&mut app, KeyCode::KeyN);
  assert!(app.world().resource::<SimOptions>().average_forces);
  tap_key(&mut app, KeyCode::KeyN);
  assert!(!app.world().resource::<SimOptions>().average_forces);
}

#[test]
fn o_cycles_overlap_modes() {
  let mut app = input_app();
  app.add_systems(Update, handle_keyboard_input);
  let mut modes = vec![];
  for _ in 0..3 {
    tap_key(&mut app, KeyCode::KeyO);
    modes.push(app.world().resource::<SimOptions>().overlap);
  }
  assert_eq!(modes, [Overlap::Resolve, Overlap::AdaptiveRange, Overlap::Spring]);
  assert!(!app.world().resource::<SimOptions>().average_forces);
}

#[test]
fn f_toggles_fullscreen() {
  let mut app = input_app();
  app.add_systems(Update, handle_keyboard_input);
  assert_eq!(primary_window_mode(&mut app), WindowMode::Windowed);

  tap_key(&mut app, KeyCode::KeyF);
  assert_eq!(
    primary_window_mode(&mut app),
    WindowMode::BorderlessFullscreen(MonitorSelection::Primary)
  );

  tap_key(&mut app, KeyCode::KeyF);
  assert_eq!(primary_window_mode(&mut app), WindowMode::Windowed);
}

// --- close_on_esc ---

fn window_count(app: &mut App) -> usize {
  app.world_mut().query::<&Window>().iter(app.world()).count()
}

#[test]
fn esc_closes_focused_window() {
  let mut app = input_app();
  app.add_systems(Update, close_on_esc);
  tap_key(&mut app, KeyCode::Escape);
  assert_eq!(window_count(&mut app), 0);
}

#[test]
fn esc_does_not_close_unfocused_window() {
  let mut app = input_app();
  app.add_systems(Update, close_on_esc);
  let window = app
    .world_mut()
    .query_filtered::<Entity, With<Window>>()
    .single(app.world())
    .unwrap();
  app.world_mut().get_mut::<Window>(window).unwrap().focused = false;
  tap_key(&mut app, KeyCode::Escape);
  assert_eq!(window_count(&mut app), 1);
}

// --- handle_mouse_input ---

fn send_wheel(app: &mut App, unit: MouseScrollUnit, y: f32) {
  let window = app.world_mut().query_filtered::<Entity, With<Window>>().single(app.world()).unwrap();
  app.world_mut().write_message(MouseWheel { unit, x: 0.0, y, window, phase: TouchPhase::Moved });
}

#[test]
fn wheel_up_zooms_in() {
  let mut app = input_app();
  app.add_systems(Update, handle_mouse_input);
  let camera = spawn_camera(&mut app, Vec3::ZERO, 1.125);
  let before = camera_scale(&app, camera);
  send_wheel(&mut app, MouseScrollUnit::Line, 1.0);
  app.update();
  assert!(camera_scale(&app, camera) < before);
}

#[test]
fn wheel_down_zooms_out() {
  let mut app = input_app();
  app.add_systems(Update, handle_mouse_input);
  let camera = spawn_camera(&mut app, Vec3::ZERO, 1.125);
  let before = camera_scale(&app, camera);
  send_wheel(&mut app, MouseScrollUnit::Line, -1.0);
  app.update();
  assert!(camera_scale(&app, camera) > before);
}

fn drag(app: &mut App, delta: Vec2) {
  app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Left);
  app.world_mut().write_message(MouseMotion { delta });
  app.update();
  clear_input(app);
}

fn camera_translation(app: &App, camera: Entity) -> Vec3 {
  app.world().get::<Transform>(camera).unwrap().translation
}

#[test]
fn ctrl_drag_moves_camera() {
  let mut app = input_app();
  app.add_systems(Update, handle_mouse_input);
  let camera = spawn_camera(&mut app, Vec3::new(0.0, 0.0, 1000.0), 1.0);
  let before = camera_translation(&app, camera);
  app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::ControlLeft);
  drag(&mut app, Vec2::new(10.0, 5.0));
  assert_ne!(camera_translation(&app, camera), before);
}

#[test]
fn drag_without_ctrl_does_not_move_camera() {
  let mut app = input_app();
  app.add_systems(Update, handle_mouse_input);
  let camera = spawn_camera(&mut app, Vec3::new(0.0, 0.0, 1000.0), 1.0);
  let before = camera_translation(&app, camera);
  drag(&mut app, Vec2::new(10.0, 5.0));
  assert_eq!(camera_translation(&app, camera), before);
}
