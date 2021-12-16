use bevy::prelude::*;
use bevy::render::{
  pipeline::{PipelineDescriptor, RenderPipeline},
  shader::{ShaderStage, ShaderStages},
};
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

pub fn init_appearances(
  mut particle_spec: ResMut<core::ParticleSpec>,
  mut meshes: ResMut<Assets<Mesh>>,
) {
  let mesh = make_circle(8.0);
  let mut rng = rand::thread_rng();
  for color in get_random_colors(particle_spec.interactions.len(), &mut rng) {
    let mut mesh_clone = mesh.clone();
    set_mesh_color(&mut mesh_clone, CORNERS as usize + 1, color);
    particle_spec.appearances.push(core::Appearance {
      color,
      mesh_handle: meshes.add(mesh_clone)
    });
  }
}

fn make_circle(diameter: f32) -> Mesh {
  let mut mesh = Mesh::new(bevy::render::pipeline::PrimitiveTopology::TriangleList);
  let mut v_pos = vec![[0.0, 0.0, 0.0]];
  v_pos.extend((0..CORNERS).map(|it| {
    let angle = it as f32 * 2.0 * PI / (CORNERS as f32);
    [angle.cos() * diameter / 2.0, angle.sin() * diameter / 2.0, 0.0]
  }));
  mesh.set_attribute(Mesh::ATTRIBUTE_POSITION, v_pos);
  let indices = (1..=CORNERS).flat_map(|it| {
    let current = it;
    let next = if it == CORNERS { 1 } else { it + 1 };
    [0u32, current as u32, next as u32]
  }).collect::<Vec<_>>();
  mesh.set_indices(Some(bevy::render::mesh::Indices::U32(indices)));
  mesh
}

fn make_hollow_circle(diameter: f32) -> Mesh {
  let mut mesh = Mesh::new(bevy::render::pipeline::PrimitiveTopology::TriangleStrip);
  let mut v_pos = Vec::new();
  v_pos.extend((0..CORNERS).flat_map(|it| {
    let angle = it as f32 * 2.0 * PI / (CORNERS as f32);
    [
      [angle.cos() * (diameter / 2.0 - 1.0), angle.sin() * (diameter / 2.0 - 1.0), 0.0],
      [angle.cos() * (diameter / 2.0 + 1.0), angle.sin() * (diameter / 2.0 + 1.0), 0.0],
    ]
  }));
  mesh.set_attribute(Mesh::ATTRIBUTE_POSITION, v_pos);
  let mut indices = vec![0, 1];
  indices.extend((1..=CORNERS).flat_map(|it| {
    let current = if it == CORNERS { 0 } else { it };
    [2 * current as u32, 2 * current as u32 + 1]
  }));
  mesh.set_indices(Some(bevy::render::mesh::Indices::U32(indices)));
  mesh
}

fn set_mesh_color(mesh: &mut Mesh, vertices: usize, color: [f32; 4]) {
  mesh.set_attribute("Vertex_Color", vec![color; vertices]);
}

pub fn init_particles(
  args: Res<ProgramArgs>,
  mut commands: Commands,
  mut pipelines: ResMut<Assets<PipelineDescriptor>>,
  mut shaders: ResMut<Assets<Shader>>,
  mut meshes: ResMut<Assets<Mesh>>,
  particle_spec: Res<core::ParticleSpec>,
  windows: Res<Windows>
) {
  let mut rng = rand::thread_rng();
  let pipeline_handle = pipelines.add(PipelineDescriptor::default_config(ShaderStages {
    vertex: shaders.add(Shader::from_glsl(ShaderStage::Vertex, VERTEX_SHADER)),
    fragment: Some(shaders.add(Shader::from_glsl(ShaderStage::Fragment, FRAGMENT_SHADER))),
  }));
  let window = windows.get_primary().expect("no primary window");
  let width = window.width();
  let height = window.height();
  let mut sim_region = core::SimRegion::new(width, height, 40.0);

  let mesh = make_hollow_circle(14.0);
  let mut selection_mesh = mesh.clone();
  let mut highlight_mesh = mesh.clone();
  set_mesh_color(&mut selection_mesh, 2 * CORNERS as usize, [1.0, 1.0, 1.0, 0.5]);
  set_mesh_color(&mut highlight_mesh, 2 * CORNERS as usize, [1.0, 0.0, 0.5, 0.5]);
  let selection_mesh_handle = meshes.add(selection_mesh);
  let highlight_mesh_handle = meshes.add(highlight_mesh);

  for _ in 0..args.num_particles {
    let interaction = core::InteractionId((0..particle_spec.interactions.len()).choose(&mut rng).expect("no particle spec"));
    let position_x = rng.gen::<f32>() * width - width / 2.0;
    let position_y = rng.gen::<f32>() * height - height / 2.0;
    let translation = Vec3::new(position_x, position_y, 0.0);
    let starting_velocity = Vec3::new(
      rng.gen::<f32>() * 250f32 - 125f32,
      rng.gen::<f32>() * 250f32 - 125f32,
      0.0);
    let particle = commands.spawn_bundle(core::ParticleBundle {
      mesh: MeshBundle {
        transform: Transform::from_translation(translation),
        mesh: particle_spec.appearances[interaction.0].mesh_handle.clone(),
        render_pipelines: RenderPipelines::from_pipelines(vec![RenderPipeline::new(
          pipeline_handle.clone(),
        )]),
        ..Default::default()
      },
      acceleration: core::Acceleration(Vec3::new(0.0, 0.0, 0.0)),
      last_pos: core::LastPosition(translation - core::DELTA_TIME as f32 * starting_velocity),
      interaction
    }).id();
    let particle_selection = commands.spawn_bundle(MeshBundle {
      visible: Visible { is_visible: false, ..Default::default() },
      mesh: selection_mesh_handle.clone(),
      render_pipelines: RenderPipelines::from_pipelines(vec![RenderPipeline::new(
        pipeline_handle.clone(),
      )]),
      ..Default::default()
    }).insert(core::Selection::default()).id();
    let particle_highlight = commands.spawn_bundle(MeshBundle {
      visible: Visible { is_visible: false, ..Default::default() },
      mesh: highlight_mesh_handle.clone(),
      render_pipelines: RenderPipelines::from_pipelines(vec![RenderPipeline::new(
        pipeline_handle.clone(),
      )]),
      ..Default::default()
    }).insert(core::Highlight::default()).id();
    commands.entity(particle).push_children(&[particle_selection, particle_highlight]);
    sim_region.insert_entity(particle, position_x, position_y);
  }
  commands.insert_resource(sim_region);
  commands.spawn_bundle(OrthographicCameraBundle::new_2d())
    .insert(core::MainCamera { zoom_base: 1.125, zoom_exponent: 1 });
}

const VERTEX_SHADER: &str = r"
#version 450
layout(location = 0) in vec3 Vertex_Position;
layout(location = 1) in vec4 Vertex_Color;
layout(location = 1) out vec4 v_Color;
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
layout(location = 1) in vec4 v_Color;
layout(location = 0) out vec4 o_Target;
void main() {
    o_Target = vec4(v_Color);
}
";