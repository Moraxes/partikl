use bevy::{
  core::FixedTimestep,
  prelude::*,
  render::{
    pipeline::{PipelineDescriptor, RenderPipeline},
    shader::{ShaderStage, ShaderStages},
  },
};
use rand::Rng;
use std::f32::consts::PI;

#[derive(Debug, Hash, PartialEq, Eq, Clone, StageLabel)]
struct FixedUpdateStage;

#[derive(Component, Default, Debug)]
struct Acceleration(Vec3);
#[derive(Component, Default, Debug)]
struct Mass(f32);
#[derive(Component, Default, Debug)]
struct LastPosition(Vec3);

#[derive(Bundle, Default)]
struct ParticleBundle {
  #[bundle]
  mesh: MeshBundle,
  mass: Mass,
  last_pos: LastPosition,
  acceleration: Acceleration,
}

const DELTA_TIME: f64 = 0.01;

fn main() {
  App::new()
    .insert_resource(Msaa { samples: 8 })
    .add_plugins(DefaultPlugins)
    .add_startup_system(generate_dots)
    .add_stage_after(
      CoreStage::Update,
      FixedUpdateStage,
      SystemStage::parallel()
        .with_run_criteria(FixedTimestep::step(DELTA_TIME))
        .with_system(compute_forces)
        .with_system(integrate)
        .with_system(update_shape),
    )
    .add_system(bevy::input::system::exit_on_esc_system)
    .run();
}

fn compute_forces(mut meshes: Query<(&mut Transform, &mut Acceleration, &Mass)>) {
  let mut iter = meshes.iter_combinations_mut();
  while let Some([(transform1, mut acceleration1, mass1), (transform2, mut acceleration2, mass2)]) = iter.fetch_next() {
    let delta = transform2.translation - transform1.translation;
    let distance_sq: f32 = delta.length_squared();

    let force = 1.0 / (distance_sq * distance_sq.sqrt());
    let force_unit_mass = delta * force;
    acceleration1.0 += force_unit_mass * mass2.0;
    acceleration2.0 -= force_unit_mass * mass1.0;
  }
}

fn integrate(mut query: Query<(&mut Acceleration, &mut Transform, &mut LastPosition)>) {
  let dt_sq = (DELTA_TIME * DELTA_TIME) as f32;
  for (mut acceleration, mut transform, mut last_pos) in query.iter_mut() {
    // verlet integration
    // x(t+dt) = 2x(t) - x(t-dt) + a(t)dt^2 + O(dt^4)

    let new_pos =
        2.0 * transform.translation - last_pos.0 + acceleration.0 * dt_sq;
    acceleration.0 = Vec3::ZERO;
    last_pos.0 = transform.translation;
    transform.translation = new_pos;
  }
}

fn update_shape(mut query: Query<(&mut Transform, &LastPosition)>) {
  for (mut transform, last_pos) in query.iter_mut() {
    let velocity = (transform.translation - last_pos.0).truncate();
    transform.scale = scale_from_velocity(velocity);
    transform.rotation = rotation_from_velocity(velocity);
  }
}

fn scale_from_velocity(velocity: Vec2) -> Vec3 {
  let velocity_length_sq = velocity.length_squared();
  let coeff = (2.0 + velocity_length_sq).log2();
  Vec3::new(2.0 * coeff, 0.75 / coeff + 0.25, 1.0)
}

fn rotation_from_velocity(velocity: Vec2) -> Quat {
  let angle = velocity.angle_between(Vec2::new(1.0, 0.0));
  Quat::from_rotation_z(-angle)
}

fn generate_dots(
  mut commands: Commands,
  mut meshes: ResMut<Assets<Mesh>>,
  mut pipelines: ResMut<Assets<PipelineDescriptor>>,
  mut shaders: ResMut<Assets<Shader>>,
  windows: Res<Windows>,
) {
  let mut rng = rand::thread_rng();
  let pipeline_handle = pipelines.add(PipelineDescriptor::default_config(ShaderStages {
    vertex: shaders.add(Shader::from_glsl(ShaderStage::Vertex, VERTEX_SHADER)),
    fragment: Some(shaders.add(Shader::from_glsl(ShaderStage::Fragment, FRAGMENT_SHADER))),
  }));

  const CORNERS: i32 = 16;
  const DIAMETER: f32 = 2.0;
  let mut mesh = Mesh::new(bevy::render::pipeline::PrimitiveTopology::TriangleList);
  let mut v_pos = vec![[0.0, 0.0, 0.0]];
  v_pos.extend((0..CORNERS).map(|it| {
    let angle = it as f32 * 2.0 * PI / (CORNERS as f32);
    [angle.cos() * DIAMETER / 2.0, angle.sin()* DIAMETER / 2.0, 0.0]
  }));
  mesh.set_attribute(Mesh::ATTRIBUTE_POSITION, v_pos);
  let mut v_color = Vec::new();
  v_color.extend_from_slice(&[[1.0, 1.0, 0.0]; (CORNERS + 1) as usize]);
  mesh.set_attribute("Vertex_Color", v_color);
  let indices = (1..CORNERS).flat_map(|it| {
    let current = it;
    let next = if it == CORNERS - 1 { 0 } else { it + 1 };
    [0u32, current as u32, next as u32]
  }).collect::<Vec<_>>();
  mesh.set_indices(Some(bevy::render::mesh::Indices::U32(indices)));
  let mesh_handle = meshes.add(mesh);

  let window = windows.get_primary().expect("No primary window.");
  let width = window.width();
  let height = window.height();

  for _ in 0..1000 {
    let position_x = rng.gen::<f32>() * width - width / 2.0;
    let position_y = rng.gen::<f32>() * height - height / 2.0;
    commands.spawn_bundle(ParticleBundle {
      mesh: MeshBundle {
        transform: Transform::from_xyz(
          position_x,
          position_y,
          0.0),
        mesh: mesh_handle.clone(),
        render_pipelines: RenderPipelines::from_pipelines(vec![RenderPipeline::new(
          pipeline_handle.clone(),
        )]),
        ..Default::default()
      },
      mass: Mass(1.0),
      acceleration: Acceleration(Vec3::new(0.0, 0.0, 0.0)),
      last_pos: LastPosition(Vec3::new(position_x, position_y, 0.0) - DELTA_TIME as f32 * Vec3::new(rng.gen::<f32>() * 250f32 - 125f32, rng.gen::<f32>() * 250f32 - 125f32, 0.0))
    });
  }
  commands.spawn_bundle(ParticleBundle {
    mesh: MeshBundle {
      transform: Transform::from_xyz(0.0, 0.0, 0.0),
      mesh: mesh_handle.clone(),
      render_pipelines: RenderPipelines::from_pipelines(vec![RenderPipeline::new(
        pipeline_handle.clone(),
      )]),
      ..Default::default()
    },
    mass: Mass(1000000.0),
    acceleration: Acceleration(Vec3::new(0.0, 0.0, 0.0)),
    last_pos: LastPosition(Vec3::ZERO)
  });

  commands.spawn_bundle(OrthographicCameraBundle::new_2d());
}

const VERTEX_SHADER: &str = r"
#version 450
layout(location = 0) in vec3 Vertex_Position;
layout(location = 1) in vec3 Vertex_Color;
layout(location = 1) out vec3 v_Color;
layout(set = 0, binding = 0) uniform CameraViewProj {
    mat4 ViewProj;
};
layout(set = 1, binding = 0) uniform Transform {
    mat4 Model;
};
void main() {
    v_Color = Vertex_Color;
    gl_Position = ViewProj * Model * vec4(Vertex_Position, 1.0);
}
";

const FRAGMENT_SHADER: &str = r"
#version 450
layout(location = 1) in vec3 v_Color;
layout(location = 0) out vec4 o_Target;
void main() {
    o_Target = vec4(v_Color, 1.0);
}
";