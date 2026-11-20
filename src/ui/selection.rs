use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::render::MainCamera;
use crate::sim::{Acceleration, SpatialGrid};

#[derive(Component, Default, Debug)]
pub struct Selection;
#[derive(Component, Default, Debug)]
pub struct Highlight;

#[derive(Default, Debug)]
pub struct SelectedGizmo {
  id: Option<Entity>,
  translation: Vec3,
}

pub fn select_on_click(
  mouse_buttons: Res<ButtonInput<MouseButton>>,
  windows: Query<&Window, With<PrimaryWindow>>,
  camera_query: Query<&Transform, With<MainCamera>>,
  particles: Query<(Entity, &Transform, &Children), With<Acceleration>>,
  grid: Res<SpatialGrid>,
  mut gizmos: Query<
    (Option<&Selection>, Option<&Highlight>, &mut Visibility),
    Or<(With<Selection>, With<Highlight>)>,
  >,
  mut selected_gizmo: Local<SelectedGizmo>,
) {
  if !mouse_buttons.just_released(MouseButton::Left) {
    return;
  }

  let window = windows.single().unwrap();
  let cursor_position_opt = window.cursor_position();
  if cursor_position_opt.is_none() {
    return;
  }
  let cursor_position = cursor_position_opt.unwrap();
  let size = Vec2::new(window.width() as f32, window.height() as f32);
  let cursor_position_offset = cursor_position - size / 2.0;
  let camera_transform = camera_query.single().unwrap();
  let world_position = camera_transform.to_matrix()
    * cursor_position_offset.extend(0.0).extend(1.0)
    * Vec4::from((1.0, -1.0, 1.0, 1.0));

  for (_, _, mut visibility) in gizmos.iter_mut().filter(|(s, _, _)| s.is_some()) {
    *visibility = Visibility::Hidden;
  }
  selected_gizmo.id = None;
  for (particle, transform, children) in particles.iter() {
    if (transform.translation - world_position.truncate())
      .truncate()
      .length_squared()
      > 16.0
    {
      continue;
    }
    selected_gizmo.id = Some(particle);
    selected_gizmo.translation = transform.translation;
    for child in children.iter() {
      if let Ok((Some(_), None, mut visibility)) = gizmos.get_mut(child) {
        *visibility = Visibility::Inherited;
      }
    }
    break;
  }

  if let Some(particle) = selected_gizmo.id {
    let (_, transform, _) = particles.get(particle).unwrap();
    selected_gizmo.translation = transform.translation;
    for (_, _, mut visibility) in gizmos.iter_mut().filter(|(_, h, _)| h.is_some()) {
      *visibility = Visibility::Hidden;
    }
    let neighbour_ids = grid
      .get_entities_by_position(selected_gizmo.translation.x, selected_gizmo.translation.y);
    for nid in neighbour_ids {
      let (_, _, children) = particles.get(nid).unwrap();
      for child in children.iter() {
        if let Ok((None, Some(_), mut visibility)) = gizmos.get_mut(child) {
          *visibility = Visibility::Inherited;
        }
      }
    }
  }
}
