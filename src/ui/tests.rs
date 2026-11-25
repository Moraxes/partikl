use super::*;
use crate::gpu::{DEFAULT_MAX_CANDIDATES, Substeps};
use crate::ui::test_util::*;
use bevy::camera::{OrthographicProjection, Projection};
use bevy::input::mouse::MouseScrollUnit;
use bevy::input::touch::TouchPhase;
use bevy::window::WindowMode;

fn tap_key(app: &mut App, key: KeyCode) {
  app
    .world_mut()
    .resource_mut::<ButtonInput<KeyCode>>()
    .press(key);
  app.update();
  clear_input(app);
  app
    .world_mut()
    .resource_mut::<ButtonInput<KeyCode>>()
    .release(key);
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
      MainCamera {
        zoom_base: 1.125,
        zoom_exponent: 1,
      },
      Transform::from_translation(translation),
      Projection::from(OrthographicProjection {
        scale,
        ..OrthographicProjection::default_2d()
      }),
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
  assert_eq!(
    *app.world().resource::<State<SimState>>().get(),
    SimState::Running
  );

  tap_key(&mut app, KeyCode::Space);
  assert_eq!(
    *app.world().resource::<State<SimState>>().get(),
    SimState::Paused
  );

  tap_key(&mut app, KeyCode::Space);
  assert_eq!(
    *app.world().resource::<State<SimState>>().get(),
    SimState::Running
  );
}

#[test]
fn unrelated_key_does_not_change_sim_state() {
  let mut app = input_app();
  app.add_systems(Update, handle_keyboard_input);
  tap_key(&mut app, KeyCode::KeyA);
  assert_eq!(
    *app.world().resource::<State<SimState>>().get(),
    SimState::Running
  );
}

/// Klawisze opcji pochodzą z deklaracji reguł: bez deklaracji N i O nic nie robią.
#[test]
fn option_keys_come_from_the_rules() {
  let mut app = input_app();
  app.insert_resource(RuleSchema::default());
  app.add_systems(Update, handle_keyboard_input);
  tap_key(&mut app, KeyCode::KeyN);
  tap_key(&mut app, KeyCode::KeyO);
  assert_eq!(*app.world().resource::<SimOptions>(), SimOptions::default());
}

#[test]
fn b_toggles_hdr_and_a_toggles_antialiasing() {
  let mut app = input_app();
  app.add_systems(Update, handle_keyboard_input);
  let quality = |app: &App| *app.world().resource::<RenderQuality>();
  // Domyślnie LDR bez wygładzania.
  assert!(!quality(&app).hdr && !quality(&app).analytic_aa);
  tap_key(&mut app, KeyCode::KeyB);
  assert!(quality(&app).hdr && !quality(&app).analytic_aa);
  tap_key(&mut app, KeyCode::KeyA);
  assert!(quality(&app).hdr && quality(&app).analytic_aa);
  tap_key(&mut app, KeyCode::KeyB);
  tap_key(&mut app, KeyCode::KeyA);
  assert!(!quality(&app).hdr && !quality(&app).analytic_aa);
}

#[test]
fn h_toggles_hud_visibility() {
  let mut app = input_app();
  app.add_systems(Update, handle_keyboard_input);
  let hud = app.world_mut().spawn((Hud, Visibility::default())).id();
  let visible = |app: &App| *app.world().get::<Visibility>(hud).unwrap() != Visibility::Hidden;
  assert!(visible(&app));
  tap_key(&mut app, KeyCode::KeyH);
  assert!(!visible(&app));
  tap_key(&mut app, KeyCode::KeyH);
  assert!(visible(&app));
}

#[test]
fn c_cycles_the_sampling_cap() {
  let mut app = input_app();
  app.add_systems(Update, handle_keyboard_input);
  let cap = |app: &App| app.world().resource::<SamplingCap>().0;
  assert_eq!(cap(&app), DEFAULT_MAX_CANDIDATES);
  let mut seen = vec![cap(&app)];
  for _ in 0..4 {
    tap_key(&mut app, KeyCode::KeyC);
    seen.push(cap(&app));
  }
  assert_eq!(seen, [256, 512, 1024, 128, 256]);
}

#[test]
fn sampling_cap_next_handles_values_off_the_cycle() {
  assert_eq!(SamplingCap(128).next(), 256);
  assert_eq!(SamplingCap(1024).next(), 128);
  // Wartość z flagi spoza cyklu przechodzi do najbliższej większej.
  assert_eq!(SamplingCap(300).next(), 512);
  assert_eq!(SamplingCap(5000).next(), 128);
}

#[test]
fn hud_text_shows_fps_and_options() {
  let schema = builtin_schema();
  let options = SimOptions::default();
  let quality = RenderQuality {
    hdr: true,
    ..RenderQuality::default()
  };
  let text = hud_text(Some(59.6), Some(100.0), &schema, &options, &quality, 256, 1);
  assert!(text.contains("60 fps"), "{text}");
  assert!(text.contains("candidates 256  C"), "{text}");
  assert!(text.contains("hdr on"), "{text}");
  assert!(text.contains("antialiasing off"), "{text}");
  assert!(
    hud_text(
      None,
      None,
      &schema,
      &SimOptions::default(),
      &RenderQuality::default(),
      DEFAULT_MAX_CANDIDATES,
      1
    )
    .contains("– fps")
  );
  // Prędkość symulacji widać tylko, gdy zwalnia.
  assert!(!text.contains("speed"), "{text}");
  let slow = hud_text(
    Some(55.0),
    Some(54.6),
    &schema,
    &options,
    &quality,
    DEFAULT_MAX_CANDIDATES,
    1,
  );
  assert!(slow.contains("sim speed 55%"), "{slow}");
}

/// Linia podkroków jest w HUD tylko dla K > 1.
#[test]
fn hud_shows_substeps_only_above_one() {
  let schema = builtin_schema();
  let hud = |k: u32| {
    hud_text(
      Some(60.0),
      None,
      &schema,
      &SimOptions::default(),
      &RenderQuality::default(),
      DEFAULT_MAX_CANDIDATES,
      k,
    )
  };
  assert!(!hud(1).contains("substeps"), "{}", hud(1));
  assert!(hud(2).contains("substeps 2  S"), "{}", hud(2));
  assert!(hud(4).contains("substeps 4  S"), "{}", hud(4));
}

#[test]
fn s_cycles_the_substeps() {
  let mut app = input_app();
  app.add_systems(Update, handle_keyboard_input);
  let substeps = |app: &App| app.world().resource::<Substeps>().0;
  assert_eq!(substeps(&app), 1);
  let mut seen = vec![substeps(&app)];
  for _ in 0..3 {
    tap_key(&mut app, KeyCode::KeyS);
    seen.push(substeps(&app));
  }
  assert_eq!(seen, [1, 2, 4, 1]);
}

#[test]
fn substeps_next_handles_values_off_the_cycle() {
  assert_eq!(Substeps(1).next(), 2);
  assert_eq!(Substeps(4).next(), 1);
  // Wartość spoza cyklu przechodzi do najbliższej większej.
  assert_eq!(Substeps(3).next(), 4);
  assert_eq!(Substeps(8).next(), 1);
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
  let window = app
    .world_mut()
    .query_filtered::<Entity, With<Window>>()
    .single(app.world())
    .unwrap();
  app.world_mut().write_message(MouseWheel {
    unit,
    x: 0.0,
    y,
    window,
    phase: TouchPhase::Moved,
  });
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
  app
    .world_mut()
    .resource_mut::<ButtonInput<MouseButton>>()
    .press(MouseButton::Left);
  app.world_mut().write_message(MouseMotion { delta });
  app.update();
  clear_input(app);
}

fn camera_translation(app: &App, camera: Entity) -> Vec3 {
  app.world().get::<Transform>(camera).unwrap().translation
}

#[test]
fn left_drag_moves_camera_with_the_content() {
  let mut app = input_app();
  app.add_systems(Update, handle_mouse_input);
  let camera = spawn_camera(&mut app, Vec3::new(0.0, 0.0, 1000.0), 1.0);
  let before = camera_translation(&app, camera);
  drag(&mut app, Vec2::new(10.0, 5.0));
  let moved = camera_translation(&app, camera) - before;
  // Zawartość podąża za kursorem: mysz w prawo i w dół (y ekranu rośnie w dół) cofa kamerę
  // w lewo i podnosi w górę świata.
  assert!(moved.x < 0.0 && moved.y > 0.0, "{moved}");
}

#[test]
fn motion_without_button_does_not_move_camera() {
  let mut app = input_app();
  app.add_systems(Update, handle_mouse_input);
  let camera = spawn_camera(&mut app, Vec3::new(0.0, 0.0, 1000.0), 1.0);
  let before = camera_translation(&app, camera);
  app.world_mut().write_message(MouseMotion {
    delta: Vec2::new(10.0, 5.0),
  });
  app.update();
  assert_eq!(camera_translation(&app, camera), before);
}

fn cursor_state(app: &mut App) -> (bool, CursorGrabMode) {
  let cursor = app
    .world_mut()
    .query_filtered::<&CursorOptions, With<PrimaryWindow>>()
    .single(app.world())
    .unwrap();
  (cursor.visible, cursor.grab_mode)
}

#[test]
fn cursor_is_hidden_and_locked_only_while_dragging() {
  let mut app = input_app();
  app.add_systems(Update, handle_mouse_input);
  spawn_camera(&mut app, Vec3::new(0.0, 0.0, 1000.0), 1.0);
  assert_eq!(cursor_state(&mut app), (true, CursorGrabMode::None));

  drag(&mut app, Vec2::new(10.0, 5.0));
  assert_eq!(cursor_state(&mut app), (false, CursorGrabMode::Locked));

  app
    .world_mut()
    .resource_mut::<ButtonInput<MouseButton>>()
    .release(MouseButton::Left);
  app.update();
  assert_eq!(cursor_state(&mut app), (true, CursorGrabMode::None));
}

#[test]
fn input_without_window_or_camera_does_not_panic() {
  // Brak kamery (okno obecne): kółko myszy i przeciąganie nie panikują.
  let mut app = input_app();
  app.add_systems(Update, (handle_keyboard_input, handle_mouse_input));
  send_wheel(&mut app, MouseScrollUnit::Line, 1.0);
  app.update();
  drag(&mut app, Vec2::new(10.0, 5.0));
  app
    .world_mut()
    .resource_mut::<ButtonInput<MouseButton>>()
    .release(MouseButton::Left);
  app.update();
  clear_input(&mut app);

  // Brak okna i brak kamery: klawisz F, kliknięcie, puszczenie, ruch i kółko myszy.
  let window = app
    .world_mut()
    .query_filtered::<Entity, With<Window>>()
    .single(app.world())
    .unwrap();
  app.world_mut().entity_mut(window).despawn();

  tap_key(&mut app, KeyCode::KeyF);
  app.world_mut().write_message(MouseWheel {
    unit: MouseScrollUnit::Line,
    x: 0.0,
    y: -1.0,
    window: Entity::PLACEHOLDER,
    phase: TouchPhase::Moved,
  });
  app.update();

  app
    .world_mut()
    .resource_mut::<ButtonInput<MouseButton>>()
    .press(MouseButton::Left);
  app.update();
  clear_input(&mut app);

  app.world_mut().write_message(MouseMotion {
    delta: Vec2::new(5.0, -3.0),
  });
  app.update();

  app
    .world_mut()
    .resource_mut::<ButtonInput<MouseButton>>()
    .release(MouseButton::Left);
  app.update();
}
