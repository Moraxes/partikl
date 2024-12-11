use bevy::app::AppExit;
use bevy::diagnostic::{Diagnostics, FrameTimeDiagnosticsPlugin};
use bevy::input::mouse::{MouseMotion, MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::render::camera::OrthographicProjection;
use bevy::window::{CursorGrabMode, PrimaryWindow, WindowMode};

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
    if time.elapsed_seconds_f64() >= time_limit {
      app_exit_events.send(AppExit);
    }
  }
}

pub fn handle_keyboard_input(
  keyboard: Res<Input<KeyCode>>,
  state: Res<State<SimState>>,
  mut next_state: ResMut<NextState<SimState>>,
  mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
  if keyboard.just_pressed(KeyCode::Space) {
    let new_state = match state.0 {
      SimState::Running => SimState::Paused,
      SimState::Paused => SimState::Running,
    };
    next_state.set(new_state);
  }
  if keyboard.just_pressed(KeyCode::F) {
    let mut primary_window = windows.get_single_mut().unwrap();
    primary_window.mode = match primary_window.mode {
      WindowMode::Windowed => WindowMode::BorderlessFullscreen,
      _ => WindowMode::Windowed,
    }
  }
}

pub fn handle_mouse_input(
  mut mouse_wheel_events: EventReader<MouseWheel>,
  mut mouse_motion_events: EventReader<MouseMotion>,
  mouse_button_input: Res<Input<MouseButton>>,
  keyboard_input: Res<Input<KeyCode>>,
  mut windows: Query<&mut Window, With<PrimaryWindow>>,
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
    let mut primary_window = windows.get_single_mut().unwrap();
    primary_window.cursor.visible = true;
    primary_window.cursor.grab_mode = CursorGrabMode::Locked;
  }

  if !keyboard_input.pressed(KeyCode::LControl) {
    return;
  }

  if mouse_button_input.just_pressed(MouseButton::Left) {
    let mut primary_window = windows.get_single_mut().unwrap();
    primary_window.cursor.visible = false;
    primary_window.cursor.grab_mode = CursorGrabMode::None;
  }

  for event in mouse_motion_events.iter() {
    if !mouse_button_input.pressed(MouseButton::Left) {
      continue;
    }
    let (_, projection, mut camera_transform) = camera.get_single_mut().unwrap();
    camera_transform.translation += (event.delta * Vec2::new(-1.0, 1.0)).extend(0.0) * projection.scale;
  }
}