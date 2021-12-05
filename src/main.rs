use bevy::{
  core::FixedTimestep,
  prelude::*,
};
use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::ecs::schedule::RunOnce;
use bevy::window::WindowMode;

mod core;
mod render;
mod sim;
mod ui;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[derive(StageLabel)]
enum Stage {
  GenerateCategories,
  FixedUpdateStage,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[derive(SystemLabel)]
enum System {
  GenerateParticles,
}

fn main() {
  App::new()
    .insert_resource(ClearColor(Color::rgb(0.0, 0.0, 0.0)))
    .insert_resource(Msaa { samples: 8 })
    .insert_resource(WindowDescriptor {
      width: 1920.0,
      height: 1080.0,
      position: None,
      resize_constraints: Default::default(),
      scale_factor_override: None,
      title: "partikl".to_string(),
      vsync: false,
      resizable: false,
      decorations: false,
      cursor_visible: false,
      cursor_locked: false,
      mode: WindowMode::BorderlessFullscreen
    })
    .add_plugins(DefaultPlugins)
    .add_plugin(FrameTimeDiagnosticsPlugin::default())
    .add_stage_before(
      CoreStage::Startup,
      Stage::GenerateCategories,
      SystemStage::single_threaded()
        .with_run_criteria(RunOnce::default())
        .with_system(render::generate_categories))
    .add_startup_system(render::generate_meshes.system().before(System::GenerateParticles))
    .add_startup_system(render::generate_particles.system().label(System::GenerateParticles))
    .add_startup_system(ui::generate_ui)
    .add_stage_after(
      CoreStage::Update,
      Stage::FixedUpdateStage,
      SystemStage::parallel()
        .with_run_criteria(FixedTimestep::step(core::DELTA_TIME))
        .with_system(sim::compute_forces)
        .with_system(sim::add_friction)
        .with_system(sim::integrate)
        .with_system(sim::wrap_around)
        .with_system(sim::update_shape),
    )
    .add_system(ui::update_text)
    .add_system(bevy::input::system::exit_on_esc_system)
    .run();
}
