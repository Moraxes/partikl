use super::*;
use crate::ui::test_util::*;

const WINDOW: Vec2 = Vec2::new(1280.0, 720.0);

fn camera_at(x: f32, y: f32) -> Transform {
  Transform::from_xyz(x, y, 1000.0)
}

struct Scene {
  app: App,
  selections: Vec<Entity>,
  highlights: Vec<Entity>,
}

/// Cząstki w podanych pozycjach, każda z dziećmi `Selection` i `Highlight`.
fn scene(positions: &[Vec2], camera: Transform) -> Scene {
  let mut app = input_app();
  app.add_systems(Update, select_on_click);
  app.world_mut().spawn((MainCamera::default(), camera));
  let mut grid = SpatialGrid::new(2560.0, 1440.0, 40.0);
  let (mut selections, mut highlights) = (vec![], vec![]);
  for p in positions {
    let particle = app
      .world_mut()
      .spawn((Acceleration::default(), Transform::from_translation(p.extend(0.0))))
      .id();
    let selection = app.world_mut().spawn((Selection, Visibility::Hidden)).id();
    let highlight = app.world_mut().spawn((Highlight, Visibility::Hidden)).id();
    app.world_mut().entity_mut(particle).add_children(&[selection, highlight]);
    grid.insert_entity(particle, p.x, p.y);
    selections.push(selection);
    highlights.push(highlight);
  }
  app.insert_resource(grid);
  Scene { app, selections, highlights }
}

fn set_cursor(app: &mut App, cursor: Option<Vec2>) {
  let mut windows = app.world_mut().query_filtered::<&mut Window, With<PrimaryWindow>>();
  windows.single_mut(app.world_mut()).unwrap().set_cursor_position(cursor);
}

fn click(app: &mut App) {
  let mut mouse = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
  mouse.press(MouseButton::Left);
  mouse.release(MouseButton::Left);
  app.update();
  clear_input(app);
}

fn visible(app: &App, entity: Entity) -> bool {
  *app.world().get::<Visibility>(entity).unwrap() != Visibility::Hidden
}

#[test]
fn click_on_particle_shows_its_selection_and_neighbours_highlight() {
  // 0 pod kursorem, 1 w tej samej okolicy, 2 daleko.
  let mut s = scene(
    &[Vec2::new(0.0, 0.0), Vec2::new(30.0, 0.0), Vec2::new(500.0, 0.0)],
    camera_at(0.0, 0.0),
  );
  set_cursor(&mut s.app, Some(WINDOW / 2.0));
  click(&mut s.app);

  assert!(visible(&s.app, s.selections[0]));
  assert!(!visible(&s.app, s.selections[1]));
  assert!(!visible(&s.app, s.selections[2]));
  assert!(visible(&s.app, s.highlights[0]), "cząstka jest własnym sąsiadem w siatce");
  assert!(visible(&s.app, s.highlights[1]));
  assert!(!visible(&s.app, s.highlights[2]));
}

#[test]
fn click_on_empty_space_hides_previous_selection() {
  let mut s = scene(&[Vec2::new(0.0, 0.0)], camera_at(0.0, 0.0));
  set_cursor(&mut s.app, Some(WINDOW / 2.0));
  click(&mut s.app);
  assert!(visible(&s.app, s.selections[0]));

  set_cursor(&mut s.app, Some(WINDOW / 2.0 + Vec2::new(300.0, 0.0)));
  click(&mut s.app);
  assert!(!visible(&s.app, s.selections[0]));
}

#[test]
fn click_without_cursor_in_window_changes_nothing() {
  let mut s = scene(&[Vec2::new(0.0, 0.0)], camera_at(0.0, 0.0));
  set_cursor(&mut s.app, None);
  click(&mut s.app);
  assert!(!visible(&s.app, s.selections[0]));
}

#[test]
fn press_without_release_does_not_select() {
  let mut s = scene(&[Vec2::new(0.0, 0.0)], camera_at(0.0, 0.0));
  set_cursor(&mut s.app, Some(WINDOW / 2.0));
  s.app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Left);
  s.app.update();
  assert!(!visible(&s.app, s.selections[0]));
}

#[test]
fn click_selects_particle_under_cursor_when_camera_is_panned_in_x() {
  let mut s = scene(&[Vec2::new(300.0, 0.0)], camera_at(300.0, 0.0));
  set_cursor(&mut s.app, Some(WINDOW / 2.0));
  click(&mut s.app);
  assert!(visible(&s.app, s.selections[0]));
}

#[test]
#[ignore = "znany błąd: znak osi y przy przesuniętej kamerze jest odwrócony w przeliczaniu \
            kursora na świat (select_on_click)"]
fn click_selects_particle_under_cursor_when_camera_is_panned_in_y() {
  let mut s = scene(&[Vec2::new(0.0, 300.0)], camera_at(0.0, 300.0));
  set_cursor(&mut s.app, Some(WINDOW / 2.0));
  click(&mut s.app);
  assert!(visible(&s.app, s.selections[0]));
}

#[test]
#[ignore = "znany błąd: select_on_click ignoruje skalę zoomu kamery (cursor_to_world nie \
            mnoży offsetu przez Projection::scale), więc klik przy zoomie trafia w złe miejsce"]
fn click_selects_particle_under_cursor_when_zoomed() {
  // Przy przybliżeniu kursor w tym samym punkcie okna wskazuje inne miejsce świata.
  let mut s = scene(&[Vec2::new(200.0, 0.0)], camera_at(0.0, 0.0));
  let camera = s.app.world_mut().query_filtered::<Entity, With<MainCamera>>().single(s.app.world()).unwrap();
  s.app.world_mut().entity_mut(camera).insert(Projection::from(OrthographicProjection {
    scale: 2.0,
    ..OrthographicProjection::default_3d()
  }));
  set_cursor(&mut s.app, Some(WINDOW / 2.0 + Vec2::new(100.0, 0.0)));
  click(&mut s.app);
  assert!(visible(&s.app, s.selections[0]));
}
