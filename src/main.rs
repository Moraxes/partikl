use bevy::{
  core::FixedTimestep,
  prelude::*,
  render::{
    pipeline::{PipelineDescriptor, RenderPipeline},
    shader::{ShaderStage, ShaderStages},
  },
};
use rand::Rng;
use rand::seq::IteratorRandom;
use std::f32::consts::PI;

#[derive(Debug, Hash, PartialEq, Eq, Clone, StageLabel)]
struct FixedUpdateStage;

#[derive(Component, Default, Debug)]
struct Acceleration(Vec3);
#[derive(Component, Default, Debug)]
struct LastPosition(Vec3);

#[derive(Bundle, Default)]
struct ParticleBundle {
  #[bundle]
  mesh: MeshBundle,
  last_pos: LastPosition,
  acceleration: Acceleration,
  category: CategoryId
}

#[derive(Default)]
struct Categories(Vec<Category>);
#[derive(Default)]
struct Category {
  force_coeffs: Vec<f32>,
  color: [f32; 3],
  mesh_handle: Handle<Mesh>
}
#[derive(Component, Default, Clone, Copy)]
struct CategoryId(usize);

const DELTA_TIME: f64 = 0.01;

fn main() {
  let mut rng = rand::thread_rng();
  let mut categories = Categories(Vec::new());
  for _ in 0..3 {
    categories.0.push(Category {
      force_coeffs: vec![100.0 * rng.gen::<f32>() - 50.0, 100.0 * rng.gen::<f32>() - 50.0, 100.0 * rng.gen::<f32>() - 50.0],
      color: [rng.gen::<f32>(), rng.gen::<f32>(), rng.gen::<f32>()],
      mesh_handle: Default::default()
    });
  }
  App::new()
    .insert_resource(Msaa { samples: 8 })
    .insert_resource(categories)
    .add_plugins(DefaultPlugins)
    .add_startup_system(generate_dots)
    .add_stage_after(
      CoreStage::Update,
      FixedUpdateStage,
      SystemStage::parallel()
        .with_run_criteria(FixedTimestep::step(DELTA_TIME))
        .with_system(compute_forces)
        .with_system(add_friction)
        .with_system(integrate)
        .with_system(update_shape),
    )
    .add_system(bevy::input::system::exit_on_esc_system)
    .run();
}

fn compute_forces(categories: Res<Categories>, mut particles: Query<(&mut Transform, &mut Acceleration, &CategoryId)>) {
  let mut iter = particles.iter_combinations_mut();
  while let Some([(transform1, mut acceleration1, cat1), (transform2, mut acceleration2, cat2)]) = iter.fetch_next() {
    let delta = transform2.translation - transform1.translation;
    let distance_sq: f32 = delta.length_squared();
    if distance_sq > 1600.0 {
      continue;
    }
    let distance = distance_sq.sqrt();
    let distance_unit_vector = delta / distance;
    // let force = 1.0 / (distance_sq * distance_sq.sqrt());
    // let force_unit_mass = delta * force;
    if distance < 10.0 {
      let safety_margin_repulsion_force = (1000.0 - 100.0 * distance) * distance_unit_vector;
      acceleration1.0 -= safety_margin_repulsion_force;
      acceleration2.0 += safety_margin_repulsion_force;
    } else {
      acceleration1.0 += zigzag_kernel(categories.0[cat2.0].force_coeffs[cat1.0], 30.0, 10.0, distance) * distance_unit_vector;
      // acceleration2.0 += attraction_force(categories.0[cat1.0].force_coeffs[cat2.0], distance) * distance_unit_vector;
    }
  }
}

fn add_friction(mut particles: Query<(&Transform, &mut LastPosition, &mut Acceleration)>) {
  for (transform, mut last_pos, mut acceleration) in particles.iter_mut() {
    let velocity = (transform.translation - last_pos.0) / DELTA_TIME as f32;
    if velocity.length_squared() < 0.0001 {
      last_pos.0 = Vec3::ZERO;
    } else {
      acceleration.0 -= velocity * 0.5;
    }
  }
}

fn zigzag_kernel(magnitude: f32, middle: f32, width: f32, x: f32) -> f32 {
  magnitude * unit_zigzag((x - middle) / width)
}

fn unit_zigzag(x: f32) -> f32 {
  triangular_kernel(-1.0, -1.0, 2.0, 3.0 * x) + triangular_kernel(1.0, 1.0, 2.0, 3.0 * x)
}

fn triangular_kernel(magnitude: f32, middle: f32, width: f32, x: f32) -> f32 {
  magnitude * unit_triangle((x - middle) / width)
}

fn unit_triangle(x: f32) -> f32 {
  (1.0 - x.abs()).max(0.0)
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
  Vec3::new(coeff, 0.75 / coeff + 0.25, 1.0)
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
  mut categories: ResMut<Categories>
) {
  let mut rng = rand::thread_rng();
  let pipeline_handle = pipelines.add(PipelineDescriptor::default_config(ShaderStages {
    vertex: shaders.add(Shader::from_glsl(ShaderStage::Vertex, VERTEX_SHADER)),
    fragment: Some(shaders.add(Shader::from_glsl(ShaderStage::Fragment, FRAGMENT_SHADER))),
  }));

  const CORNERS: i32 = 16;
  const DIAMETER: f32 = 4.0;
  let mut mesh = Mesh::new(bevy::render::pipeline::PrimitiveTopology::TriangleList);
  let mut v_pos = vec![[0.0, 0.0, 0.0]];
  v_pos.extend((0..CORNERS).map(|it| {
    let angle = it as f32 * 2.0 * PI / (CORNERS as f32);
    [angle.cos() * DIAMETER / 2.0, angle.sin() * DIAMETER / 2.0, 0.0]
  }));
  mesh.set_attribute(Mesh::ATTRIBUTE_POSITION, v_pos);
  for mut cat in &mut categories.0 {
    let mut mesh_clone = mesh.clone();
    let mut v_color = Vec::new();
    v_color.extend_from_slice(&[cat.color; (CORNERS + 1) as usize]);
    mesh_clone.set_attribute("Vertex_Color", v_color);
    let indices = (1..=CORNERS).flat_map(|it| {
      let current = it;
      let next = if it == CORNERS { 1 } else { it + 1 };
      [0u32, current as u32, next as u32]
    }).collect::<Vec<_>>();
    mesh_clone.set_indices(Some(  bevy::render::mesh::Indices::U32(indices)));
    cat.mesh_handle = meshes.add(mesh_clone);
  }

  let window = windows.get_primary().expect("No primary window.");
  let width = window.width();
  let height = window.height();

  for _ in 0..1000 {
    let category = CategoryId((0..categories.0.len()).choose(&mut rng).expect("no categories"));
    let position_x = rng.gen::<f32>() * width - width / 2.0;
    let position_y = rng.gen::<f32>() * height - height / 2.0;
    commands.spawn_bundle(ParticleBundle {
      mesh: MeshBundle {
        transform: Transform::from_xyz(
          position_x,
          position_y,
          0.0),
        mesh: categories.0[category.0].mesh_handle.clone(),
        render_pipelines: RenderPipelines::from_pipelines(vec![RenderPipeline::new(
          pipeline_handle.clone(),
        )]),
        ..Default::default()
      },
      acceleration: Acceleration(Vec3::new(0.0, 0.0, 0.0)),
      last_pos: LastPosition(Vec3::new(position_x, position_y, 0.0) - DELTA_TIME as f32 * Vec3::new(rng.gen::<f32>() * 250f32 - 125f32, rng.gen::<f32>() * 250f32 - 125f32, 0.0)),
      category
    });
  }

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