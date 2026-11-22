use bevy::app::AppExit;
use bevy::color::palettes::css::WHITE;
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::input::mouse::{MouseMotion, MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow, WindowMode};

use crate::args::ProgramArgs;
use crate::gpu::{RenderQuality, SIM_SPEED, SamplingCap};
use crate::options::{RuleSchema, SimOptions, SimState};

pub struct UiPlugin;

impl Plugin for UiPlugin {
  fn build(&self, app: &mut App) {
    app
      .add_plugins(FrameTimeDiagnosticsPlugin::default())
      .add_systems(Startup, init_ui)
      .add_systems(
        Update,
        (
          update_text,
          exit_after_time,
          handle_keyboard_input,
          handle_mouse_input,
          close_on_esc,
        ),
      );
  }
}

/// Kamera symulacji; zoom kółkiem myszy: skala rzutu `zoom_base^zoom_exponent`.
#[derive(Component, Default, Debug)]
pub struct MainCamera {
  pub zoom_exponent: i32,
  pub zoom_base: f32,
}

/// Napisy w prawym dolnym rogu: FPS i stan opcji symulacji. Przełączane klawiszem H.
#[derive(Component)]
pub struct Hud;

/// Font napisów, wbudowany w binarkę (bez katalogu `assets` obok pliku wykonywalnego).
const HUD_FONT: &[u8] = include_bytes!("../../assets/FiraMono-Regular.ttf");

pub fn init_ui(mut commands: Commands, mut fonts: ResMut<Assets<Font>>) {
  let font = fonts.add(Font::from_bytes(HUD_FONT.to_vec()));
  commands.spawn((
    Hud,
    Text::new(""),
    TextFont {
      font: FontSource::Handle(font),
      font_size: FontSize::Px(13.0),
      ..Default::default()
    },
    TextColor(WHITE.with_alpha(0.4).into()),
    TextLayout::justify(Justify::Right),
    Node {
      position_type: PositionType::Absolute,
      right: Val::Px(12.0),
      bottom: Val::Px(10.0),
      ..Default::default()
    },
  ));
}

/// Treść napisów: FPS (`None`, dopóki nie ma średniej), prędkość symulacji w procentach czasu
/// rzeczywistego (tylko gdy zwalnia poniżej 97%), opcje z klawiszami i próg próbkowania `candidates`.
pub fn hud_text(
  fps: Option<f64>,
  speed: Option<f64>,
  schema: &RuleSchema,
  options: &SimOptions,
  quality: &RenderQuality,
  candidates: u32,
) -> String {
  let fps = fps.map_or("–".to_string(), |fps| format!("{fps:.0}"));
  let speed = speed.filter(|speed| *speed < 97.0).map_or(String::new(), |speed| format!("sim speed {speed:.0}%\n"));
  let on_off = |on: bool| if on { "on" } else { "off" };
  // Opcje reguł z klawiszami zadeklarowanymi przez moduł reguł.
  let rule_options: String = schema
    .options
    .iter()
    .map(|decl| {
      let value = options.choice(decl).label.replace('_', " ");
      format!("{} {value}  {}\n", decl.title(), decl.key.map_or(String::new(), String::from))
    })
    .collect();
  format!(
    "{fps} fps\n{speed}{rule_options}candidates {candidates}  C\nhdr {}  B\nantialiasing {}  A\nhide  H",
    on_off(quality.hdr),
    on_off(quality.analytic_aa),
  )
}

pub fn update_text(
  diagnostics: Res<DiagnosticsStore>,
  schema: Res<RuleSchema>,
  options: Res<SimOptions>,
  quality: Res<RenderQuality>,
  cap: Res<SamplingCap>,
  query: Query<Entity, With<Hud>>,
  mut writer: TextUiWriter,
) {
  let Ok(entity) = query.single() else { return };
  let fps = diagnostics.get(&FrameTimeDiagnosticsPlugin::FPS).and_then(|fps| fps.average());
  let speed = diagnostics.get(&SIM_SPEED).and_then(|speed| speed.value());
  let text = hud_text(fps, speed, &schema, &options, &quality, cap.0);
  let mut current = writer.text(entity, 0);
  // Bez zapisu, gdy bez zmian: zapis oznacza tekst do ponownego układu.
  if *current != text {
    *current = text;
  }
}

pub fn close_on_esc(
  mut commands: Commands,
  focused_windows: Query<(Entity, &Window)>,
  input: Res<ButtonInput<KeyCode>>,
) {
  for (window, focus) in focused_windows.iter() {
    if !focus.focused {
      continue;
    }

    if input.just_pressed(KeyCode::Escape) {
      commands.entity(window).despawn();
    }
  }
}

pub fn exit_after_time(
  args: Res<ProgramArgs>,
  time: Res<Time>,
  mut app_exit_events: MessageWriter<AppExit>,
) {
  if let Some(time_limit) = args.exit_after {
    if time.elapsed_secs_f64() >= time_limit {
      app_exit_events.write(AppExit::Success);
    }
  }
}

#[allow(clippy::too_many_arguments)]
pub fn handle_keyboard_input(
  keyboard: Res<ButtonInput<KeyCode>>,
  state: Res<State<SimState>>,
  mut next_state: ResMut<NextState<SimState>>,
  schema: Res<RuleSchema>,
  mut options: ResMut<SimOptions>,
  mut quality: ResMut<RenderQuality>,
  mut cap: ResMut<SamplingCap>,
  mut windows: Query<&mut Window, With<PrimaryWindow>>,
  mut hud: Query<&mut Visibility, With<Hud>>,
) {
  if keyboard.just_pressed(KeyCode::Space) {
    let new_state = match state.get() {
      SimState::Running => SimState::Paused,
      SimState::Paused => SimState::Running,
    };
    next_state.set(new_state);
  }
  // Klawisze opcji reguł deklaruje moduł reguł.
  for decl in &schema.options {
    if decl.keycode().is_some_and(|key| keyboard.just_pressed(key)) {
      options.cycle(decl);
    }
  }
  if keyboard.just_pressed(KeyCode::KeyB) {
    quality.hdr = !quality.hdr;
  }
  if keyboard.just_pressed(KeyCode::KeyA) {
    quality.analytic_aa = !quality.analytic_aa;
  }
  if keyboard.just_pressed(KeyCode::KeyC) {
    cap.0 = cap.next();
  }
  if keyboard.just_pressed(KeyCode::KeyH) {
    for mut visibility in hud.iter_mut() {
      *visibility = match *visibility {
        Visibility::Hidden => Visibility::Inherited,
        _ => Visibility::Hidden,
      };
    }
  }
  if keyboard.just_pressed(KeyCode::KeyF) {
    let mut primary_window = windows.single_mut().unwrap();
    primary_window.mode = match primary_window.mode {
      WindowMode::Windowed => WindowMode::BorderlessFullscreen(MonitorSelection::Primary),
      _ => WindowMode::Windowed,
    }
  }
}

pub fn handle_mouse_input(
  mut mouse_wheel_events: MessageReader<MouseWheel>,
  mut mouse_motion_events: MessageReader<MouseMotion>,
  mouse_button_input: Res<ButtonInput<MouseButton>>,
  mut cursor_options: Query<&mut CursorOptions, With<PrimaryWindow>>,
  mut camera: Query<(&mut MainCamera, &mut Projection, &mut Transform)>,
) {
  for event in mouse_wheel_events.read() {
    let log_delta = match event {
      MouseWheel {
        unit: MouseScrollUnit::Line,
        y,
        ..
      } => y.round() as i32,
      MouseWheel {
        unit: MouseScrollUnit::Pixel,
        y,
        ..
      } => (y / 10.0).round() as i32,
    };
    let (mut main_camera, mut projection, _) = camera.single_mut().unwrap();
    main_camera.zoom_exponent -= log_delta;
    if let Projection::Orthographic(projection) = &mut *projection {
      projection.scale = main_camera.zoom_base.powf(main_camera.zoom_exponent as f32);
    }
  }

  // Przeciąganie lewym przyciskiem: kursor zablokowany i ukryty, dopóki przycisk jest wciśnięty.
  if mouse_button_input.just_pressed(MouseButton::Left) {
    let mut cursor = cursor_options.single_mut().unwrap();
    cursor.visible = false;
    cursor.grab_mode = CursorGrabMode::Locked;
  }
  if mouse_button_input.just_released(MouseButton::Left) {
    let mut cursor = cursor_options.single_mut().unwrap();
    cursor.visible = true;
    cursor.grab_mode = CursorGrabMode::None;
  }

  for event in mouse_motion_events.read() {
    if !mouse_button_input.pressed(MouseButton::Left) {
      continue;
    }
    let (_, projection, mut camera_transform) = camera.single_mut().unwrap();
    if let Projection::Orthographic(projection) = &*projection {
      camera_transform.translation +=
        (event.delta * Vec2::new(-1.0, 1.0)).extend(0.0) * projection.scale;
    }
  }
}

#[cfg(test)]
pub(crate) mod test_util;
#[cfg(test)]
mod tests;
