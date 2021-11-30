use bevy::{
  core::FixedTimestep,
  prelude::*,
};
use bevy::ecs::schedule::RunOnce;

mod core;
mod render;
mod sim;

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
    .add_plugins(DefaultPlugins)
    .add_stage_before(
      CoreStage::Startup,
      Stage::GenerateCategories,
      SystemStage::single_threaded()
        .with_run_criteria(RunOnce::default())
        .with_system(render::generate_categories))
    .add_startup_system(render::generate_meshes.system().before(System::GenerateParticles))
    .add_startup_system(render::generate_particles.system().label(System::GenerateParticles))
    .add_startup_system(render::generate_ui)
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
    .add_system(bevy::input::system::exit_on_esc_system)
    .run();
}
