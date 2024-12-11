use bevy::app::AppExit;
use bevy::diagnostic::{Diagnostics, FrameTimeDiagnosticsPlugin};
use bevy::input::mouse::{MouseMotion, MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::render::camera::OrthographicProjection;
use bevy::window::WindowMode;

use crate::core::*;

#[derive(Component)]
pub struct FpsText;

pub fn init_ui(mut commands: Commands, asset_server: Res<AssetServer>) {
  commands.spawn(TextBundle::from_section("hello", TextStyle {
    font: asset_server.load("FiraMono-Regular.ttf"),
      font_size: 16.0,
      color: Color::WHITE,
    })).insert(FpsText);
}

pub fn update_text(diagnostics: Res<Diagnostics>, mut query: Query<&mut Text, With<FpsText>>) {
  for mut text in query.iter_mut() {
    if let Some(fps) = diagnostics.get(FrameTimeDiagnosticsPlugin::FPS) {
      if let Some(average) = fps.average() {
        text.sections[0].value = format!("{:.2}", average);
      }
    }
  }
}

pub fn exit_after_time(
  args: Res<ProgramArgs>,
  time: Res<Time>,
  mut app_exit_events: EventWriter<AppExit>,
) {
  if let Some(time_limit) = args.exit_after {
    if time.seconds_since_startup() >= time_limit {
      app_exit_events.send(AppExit);
    }
  }
}

pub fn handle_keyboard_input(
  keyboard: Res<Input<KeyCode>>,
  mut state: ResMut<State<SimState>>,
  mut windows: ResMut<Windows>
) {
  if keyboard.just_pressed(KeyCode::Space) {
    let new_state = match state.current() {
      SimState::Running => SimState::Paused,
      SimState::Paused => SimState::Running,
    };
    state.set(new_state).unwrap();
  }
  if keyboard.just_pressed(KeyCode::F) {
    let primary_window = windows.get_primary_mut().unwrap();
    match primary_window.mode() {
      WindowMode::Windowed => primary_window.set_mode(WindowMode::BorderlessFullscreen),
      _ => primary_window.set_mode(WindowMode::Windowed),
    }
  }
}

pub fn handle_mouse_input(
  mut mouse_wheel_events: EventReader<MouseWheel>,
  mut mouse_motion_events: EventReader<MouseMotion>,
  mouse_button_input: Res<Input<MouseButton>>,
  keyboard_input: Res<Input<KeyCode>>,
  mut windows: ResMut<Windows>,
  mut camera: Query<(&mut MainCamera, &mut OrthographicProjection, &mut Transform)>
) {
  for event in mouse_wheel_events.iter() {
    let log_delta = match event {
      MouseWheel { unit: MouseScrollUnit::Line, y, .. } => y.round() as i32,
      MouseWheel { unit: MouseScrollUnit::Pixel, y, .. } => (y / 10.0).round() as i32,
    };
    let (mut main_camera, mut projection, _) = camera.get_single_mut().unwrap();
    main_camera.zoom_exponent -= log_delta;
    projection.scale = main_camera.zoom_base.powf(main_camera.zoom_exponent as f32);
  }

  if mouse_button_input.just_released(MouseButton::Left) {
    let primary_window = windows.get_primary_mut().unwrap();
    primary_window.set_cursor_visibility(true);
    primary_window.set_cursor_lock_mode(true);
  }

  if !keyboard_input.pressed(KeyCode::LControl) {
    return;
  }

  if mouse_button_input.just_pressed(MouseButton::Left) {
    let primary_window = windows.get_primary_mut().unwrap();
    primary_window.set_cursor_visibility(false);
    primary_window.set_cursor_lock_mode(false);
  }

  for event in mouse_motion_events.iter() {
    if !mouse_button_input.pressed(MouseButton::Left) {
      continue;
    }
    let (_, projection, mut camera_transform) = camera.get_single_mut().unwrap();
    camera_transform.translation += (event.delta * Vec2::new(-1.0, 1.0)).extend(0.0) * projection.scale;
  }
}