use bevy::core_pipeline::clear_color::ClearColorConfig;
use bevy::prelude::*;
use bevy::render::camera::ScalingMode;
use bevy::window::PrimaryWindow;
use rand::prelude::*;
use std::f32::consts::PI;
use crate::args::ProgramArgs;

use crate::core;

const CORNERS: i32 = 16;

fn get_random_colors(n: usize, rng: &mut impl Rng) -> Vec<[f32; 4]> {
  let phase = 360.0 * rng.gen::<f32>();
  (0..n).map(|it| {
    let mut hue = phase + 360.0 * it as f32 / n as f32;
    if hue > 0.0 {
      hue -= 360.0;
    }
    Color::hsl(hue,
      0.5 + 0.5 * rng.gen::<f32>(),
      0.25 + 0.5 * rng.gen::<f32>()
    ).as_rgba_f32()
  }).collect()
}

pub fn init_materials(
  mut particle_spec: ResMut<core::ParticleSpec>,
  mut materials: ResMut<Assets<StandardMaterial>>,
) {
  let mut rng = thread_rng();
  for color in get_random_colors(particle_spec.interactions.len(), &mut rng) {
    let material = StandardMaterial {
      base_color: color.into(),
      double_sided: true,
      unlit: true,
      ..Default::default()
    };
    particle_spec.materials.push(materials.add(material));
  }
}

#[allow(dead_code)]
fn make_circle(diameter: f32) -> Mesh {
  use bevy::render::{mesh::Indices, render_resource::PrimitiveTopology};
  let mut mesh = Mesh::new(PrimitiveTopology::TriangleList);
  let mut v_pos = vec![[0.0, 0.0, 0.0]];
  v_pos.extend((0..CORNERS).map(|it| {
    let angle = it as f32 * 2.0 * PI / (CORNERS as f32);
    [angle.cos() * diameter / 2.0, angle.sin() * diameter / 2.0, 0.0]
  }));
  mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, v_pos);
  let indices = (1..=CORNERS).flat_map(|it| {
    let current = it;
    let next = if it == CORNERS { 1 } else { it + 1 };
    [0u32, current as u32, next as u32]
  }).collect::<Vec<_>>();
  mesh.set_indices(Some(Indices::U32(indices)));
  mesh
}

#[allow(dead_code)]
fn make_hollow_circle(diameter: f32) -> Mesh {
  use bevy::render::{mesh::Indices, render_resource::PrimitiveTopology};
  let mut mesh = Mesh::new(PrimitiveTopology::TriangleStrip);
  let mut v_pos = Vec::new();
  v_pos.extend((0..CORNERS).flat_map(|it| {
    let angle = it as f32 * 2.0 * PI / (CORNERS as f32);
    [
      [angle.cos() * (diameter / 2.0 - 1.0), angle.sin() * (diameter / 2.0 - 1.0), 0.0],
      [angle.cos() * (diameter / 2.0 + 1.0), angle.sin() * (diameter / 2.0 + 1.0), 0.0],
    ]
  }));
  mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, v_pos);
  let mut indices = vec![0, 1];
  indices.extend((1..=CORNERS).flat_map(|it| {
    let current = if it == CORNERS { 0 } else { it };
    [2 * current as u32, 2 * current as u32 + 1]
  }));
  mesh.set_indices(Some(Indices::U32(indices)));
  mesh
}

pub fn init_particles(
  args: Res<ProgramArgs>,
  mut commands: Commands,
  mut meshes: ResMut<Assets<Mesh>>,
  mut materials: ResMut<Assets<StandardMaterial>>,
  particle_spec: Res<core::ParticleSpec>,
  windows: Query<&Window, With<PrimaryWindow>>,
) {
  let mut rng = thread_rng();
  let window = windows.get_single().expect("no primary window");
  let width = window.width();
  let height = window.height();
  let mut sim_region = core::SimRegion::new(width, height, 40.0);

  let circle_mesh = meshes.add(
    Mesh::try_from(shape::Icosphere { radius: 4.0, subdivisions: 2 }).unwrap());
  let gizmo_mesh = meshes.add(
    Mesh::try_from(shape::Icosphere { radius: 7.0, subdivisions: 2 }).unwrap());

  for _ in 0..args.num_particles {
    let interaction = core::InteractionId((0..particle_spec.interactions.len()).choose(&mut rng).expect("no particle spec"));
    let position_x = rng.gen::<f32>() * width - width / 2.0;
    let position_y = rng.gen::<f32>() * height - height / 2.0;
    let translation = Vec3::new(position_x, position_y, 0.0);
    let starting_velocity = Vec3::new(
      rng.gen::<f32>() * 250f32 - 125f32,
      rng.gen::<f32>() * 250f32 - 125f32,
      0.0);
    let particle = commands.spawn(core::ParticleBundle {
      mesh: PbrBundle {
        transform: Transform::from_translation(translation),
        mesh: circle_mesh.clone(),
        material: particle_spec.materials[interaction.0].clone(),
        ..Default::default()
      },
      acceleration: core::Acceleration(Vec2::new(0.0, 0.0)),
      last_pos: core::LastPosition((translation - core::DELTA_TIME as f32 * starting_velocity).truncate()),
      interaction
    }).id();
    let particle_selection = commands.spawn(PbrBundle {
      visibility: Visibility::Hidden,
      mesh: gizmo_mesh.clone(),
      material: materials.add(Color::from([1.0, 1.0, 1.0, 0.5]).into()),
      ..Default::default()
    }).insert(core::Selection::default()).id();
    let particle_highlight = commands.spawn(PbrBundle {
      visibility: Visibility::Hidden,
      mesh: gizmo_mesh.clone(),
      material: materials.add(Color::from([1.0f32, 0.0, 0.5, 0.5]).into()),
      ..Default::default()
    }).insert(core::Highlight::default()).id();
    commands.entity(particle).push_children(&[particle_selection, particle_highlight]);
    sim_region.insert_entity(particle, position_x, position_y);
  }
  commands.insert_resource(sim_region);

  let mut camera = Camera3dBundle {
    projection: OrthographicProjection {
        scaling_mode: ScalingMode::WindowSize,
        ..default()
    }.into(),
    ..Default::default()
  };
  camera.camera_3d.clear_color = ClearColorConfig::Custom(Color::BLACK);
  camera.transform = Transform::from_xyz(0.0, 0.0, 1000.0).looking_at(Vec3::ZERO, Vec3::Y);
  commands.spawn(camera)
    .insert(core::MainCamera { zoom_base: 1.125, zoom_exponent: 1 });
}
