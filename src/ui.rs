use bevy::diagnostic::{Diagnostics, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;

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