use bevy::prelude::*;
use bevy::render::{
  pipeline::{PipelineDescriptor, RenderPipeline},
  shader::{ShaderStage, ShaderStages},
};
use rand::prelude::*;
use std::f32::consts::PI;
use crate::args::ProgramArgs;

use crate::core;

fn get_random_colors(n: usize, rng: &mut impl Rng) -> Vec<[f32; 3]> {
  let phase = 360.0 * rng.gen::<f32>();
  (0..n).map(|it| {
    let mut hue = phase + 360.0 * it as f32 / n as f32;
    if hue > 0.0 {
      hue -= 360.0;
    }
    let rgba = Color::hsl(hue,
      0.5 + 0.5 * rng.gen::<f32>(),
      0.25 + 0.5 * rng.gen::<f32>()
    ).as_rgba_f32();
    [rgba[0], rgba[1], rgba[2]]
  }).collect()
}

pub fn init_categories(
  mut commands: Commands,
) {
  let mut rng = rand::thread_rng();
  let mut categories = core::Categories(Vec::new());
  for col in get_random_colors(3, &mut rng) {
    let category = core::Category {
      force_coeffs: vec![1000.0 * rng.gen::<f32>() - 500.0, 1000.0 * rng.gen::<f32>() - 500.0, 1000.0 * rng.gen::<f32>() - 500.0],
      color: col,
      mesh_handle: Default::default()
    };
    categories.0.push(category);
  }
  commands.insert_resource(categories);
}

pub fn init_meshes(
  mut meshes: ResMut<Assets<Mesh>>,
  mut categories: ResMut<core::Categories>,
) {
  const CORNERS: i32 = 16;
  const DIAMETER: f32 = 8.0;
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
    mesh_clone.set_indices(Some(bevy::render::mesh::Indices::U32(indices)));
    cat.mesh_handle = meshes.add(mesh_clone);
  }
}

pub fn init_particles(
  args: Res<ProgramArgs>,
  mut commands: Commands,
  mut pipelines: ResMut<Assets<PipelineDescriptor>>,
  mut shaders: ResMut<Assets<Shader>>,
  categories: Res<core::Categories>,
  windows: Res<Windows>
) {
  let mut rng = rand::thread_rng();
  let pipeline_handle = pipelines.add(PipelineDescriptor::default_config(ShaderStages {
    vertex: shaders.add(Shader::from_glsl(ShaderStage::Vertex, VERTEX_SHADER)),
    fragment: Some(shaders.add(Shader::from_glsl(ShaderStage::Fragment, FRAGMENT_SHADER))),
  }));
  let mut spatial_index = core::SpatialIndex {
    granularity: 40.0,
    ..Default::default()
  };
  let window = windows.get_primary().expect("no primary window");
  let width = window.width();
  let height = window.height();

  for _ in 0..args.number_of_particles {
    let category = core::CategoryId((0..categories.0.len()).choose(&mut rng).expect("no categories"));
    let position_x = rng.gen::<f32>() * width - width / 2.0;
    let position_y = rng.gen::<f32>() * height - height / 2.0;
    let translation = Vec3::new(position_x, position_y, 0.0);
    let starting_velocity = Vec3::new(
      rng.gen::<f32>() * 250f32 - 125f32,
      rng.gen::<f32>() * 250f32 - 125f32,
      0.0);
    let entity = commands.spawn_bundle(core::ParticleBundle {
      mesh: MeshBundle {
        transform: Transform::from_translation(translation),
        mesh: categories.0[category.0].mesh_handle.clone(),
        render_pipelines: RenderPipelines::from_pipelines(vec![RenderPipeline::new(
          pipeline_handle.clone(),
        )]),
        ..Default::default()
      },
      acceleration: core::Acceleration(Vec3::new(0.0, 0.0, 0.0)),
      last_pos: core::LastPosition(translation - core::DELTA_TIME as f32 * starting_velocity),
      category
    }).id();
    spatial_index.insert_entity(entity, position_x, position_y);
  }
  commands.insert_resource(core::SimRegion::new(width, height));
  commands.insert_resource(spatial_index);
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