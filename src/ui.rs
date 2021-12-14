use bevy::app::AppExit;
use bevy::diagnostic::{Diagnostics, FrameTimeDiagnosticsPlugin};
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::render::camera::OrthographicProjection;
use bevy::window::WindowMode;

use crate::core::*;

#[derive(Component)]
pub struct FpsText;

pub fn init_ui(mut commands: Commands, asset_server: Res<AssetServer>) {
  commands.spawn_bundle(UiCameraBundle::default());
  commands.spawn_bundle(TextBundle {
    style: Style {
      align_self: AlignSelf::FlexEnd,
      position_type: PositionType::Absolute,
      position: Rect {
        bottom: Val::Px(5.0),
        top: Val::Px(5.0),
        ..Default::default()
      },
      ..Default::default()
    },
    text: Text::with_section(
      "hello",
      TextStyle {
      font: asset_server.load("FiraMono-Regular.ttf"),
        font_size: 16.0,
        color: Color::WHITE,
      },
      TextAlignment {
        horizontal: HorizontalAlign::Center,
        ..Default::default()
      }),
    ..Default::default()
  }).insert(FpsText);
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
  mut camera: Query<(&mut MainCamera, &mut OrthographicProjection)>
) {
  for event in mouse_wheel_events.iter() {
    let log_delta = match event {
      MouseWheel { unit: MouseScrollUnit::Line, y, .. } => y.round() as i32,
      MouseWheel { unit: MouseScrollUnit::Pixel, y, .. } => (y / 10.0).round() as i32,
    };
    let (mut main_camera, mut projection) = camera.get_single_mut().unwrap();
    main_camera.zoom_exponent -= log_delta;
    projection.scale = main_camera.zoom_base.powf(main_camera.zoom_exponent as f32);
  }
}