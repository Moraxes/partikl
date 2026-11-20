use crate::args::ProgramArgs;
use bevy::prelude::*;
use bevy::camera::{ClearColorConfig, Hdr, ScalingMode};
use bevy::window::PrimaryWindow;
use rand::prelude::*;

use crate::config::{DELTA_TIME, INTERACTION_RADIUS};
use crate::sim::{Acceleration, InteractionId, LastPosition, ParticleBundle, SpatialGrid};
use crate::spec::ParticleSpec;
use crate::ui::selection::{Highlight, Selection};

/// Materiały cząstek, indeksowane `InteractionId`.
#[derive(Resource, Default)]
pub struct ParticleMaterials(pub Vec<Handle<StandardMaterial>>);

#[derive(Component, Default, Debug)]
pub struct MainCamera {
  pub zoom_exponent: i32,
  pub zoom_base: f32,
}

pub struct RenderPlugin;

impl Plugin for RenderPlugin {
  fn build(&self, app: &mut App) {
    app.add_systems(Startup, (init_materials, init_particles).chain());
  }
}

fn get_random_colors(n: usize, rng: &mut impl Rng) -> Vec<Color> {
  let phase = 360.0 * rng.random::<f32>();
  let spread = 60.0 + 300.0 * rng.random::<f32>();
  (0..n)
    .map(|it| {
      let mut hue = phase + spread * it as f32 / n as f32;
      if hue > 360.0 {
        hue -= 360.0;
      }
      Color::lch(
        0.5 + 0.5 * rng.random::<f32>(),
        0.325 + 0.5 * rng.random::<f32>(),
        hue,
      )
      .into()
    })
    .collect()
}

pub fn init_materials(
  mut commands: Commands,
  particle_spec: Res<ParticleSpec>,
  mut materials: ResMut<Assets<StandardMaterial>>,
) {
  let mut rng = rand::rng();
  let handles = get_random_colors(particle_spec.interactions.len(), &mut rng)
    .into_iter()
    .map(|color| {
    let material = StandardMaterial {
      base_color: color,
      double_sided: true,
      unlit: true,
      ..Default::default()
    };
    materials.add(material)
    })
    .collect();
  commands.insert_resource(ParticleMaterials(handles));
}

pub fn init_particles(
  args: Res<ProgramArgs>,
  mut commands: Commands,
  mut meshes: ResMut<Assets<Mesh>>,
  mut materials: ResMut<Assets<StandardMaterial>>,
  particle_spec: Res<ParticleSpec>,
  particle_materials: Res<ParticleMaterials>,
  windows: Query<&Window, With<PrimaryWindow>>,
) {
  let mut rng = rand::rng();
  let window = windows.single().expect("no primary window");
  let width = window.width();
  let height = window.height();
  let mut grid = SpatialGrid::new(width, height, INTERACTION_RADIUS);

  let circle_mesh = meshes.add(Mesh::try_from(Sphere::new(args.particle_size)).unwrap());
  let gizmo_mesh = meshes.add(Mesh::try_from(Sphere::new(args.particle_size + 3.0)).unwrap());
  let mut placed = Vec::with_capacity(args.num_particles);

  for _ in 0..args.num_particles {
    let interaction = InteractionId(
      (0..particle_spec.interactions.len())
        .choose(&mut rng)
        .expect("no particle spec"),
    );
    let position_x = rng.random::<f32>() * width - width / 2.0;
    let position_y = rng.random::<f32>() * height - height / 2.0;
    let translation = Vec3::new(position_x, position_y, 0.0);
    let starting_velocity = Vec3::new(
      rng.random::<f32>() * 250f32 - 125f32,
      rng.random::<f32>() * 250f32 - 125f32,
      0.0,
    );
    let particle = commands
      .spawn((
        ParticleBundle {
          acceleration: Acceleration(Vec2::new(0.0, 0.0)),
          last_pos: LastPosition(
            (translation - DELTA_TIME as f32 * starting_velocity).truncate(),
          ),
          interaction,
        },
        Transform::from_translation(translation),
        Mesh3d(circle_mesh.clone()),
        MeshMaterial3d(particle_materials.0[interaction.0].clone()),
      ))
      .id();
    let particle_selection = commands
      .spawn((
        Selection,
        Visibility::Hidden,
        Mesh3d(gizmo_mesh.clone()),
        MeshMaterial3d(materials.add(Color::srgba(1.0, 1.0, 1.0, 0.5))),
      ))
      .id();
    let particle_highlight = commands
      .spawn((
        Highlight,
        Visibility::Hidden,
        Mesh3d(gizmo_mesh.clone()),
        MeshMaterial3d(materials.add(Color::srgba(1.0f32, 0.0, 0.5, 0.5))),
      ))
      .id();
    commands
      .entity(particle)
      .add_children(&[particle_selection, particle_highlight]);
    placed.push((particle, Vec2::new(position_x, position_y), interaction));
  }
  grid.rebuild(placed);
  commands.insert_resource(grid);

  commands.spawn((
    MainCamera {
      zoom_base: 1.125,
      zoom_exponent: 1,
    },
    Camera3d::default(),
    Hdr,
    Camera {
      clear_color: ClearColorConfig::Custom(Color::BLACK),
      ..Default::default()
    },
    Msaa::Sample4,
    Transform::from_xyz(0.0, 0.0, 1000.0).looking_at(Vec3::ZERO, Vec3::Y),
    Projection::from(OrthographicProjection {
      scaling_mode: ScalingMode::WindowSize,
      ..OrthographicProjection::default_3d()
    }),
  ));
}
