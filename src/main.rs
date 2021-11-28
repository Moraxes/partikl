use bevy::{
  core::FixedTimestep,
  prelude::*,
};

mod core;
mod render;
mod sim;

#[derive(Debug, Hash, PartialEq, Eq, Clone, StageLabel)]
struct FixedUpdateStage;

fn main() {
  App::new()
    .insert_resource(Msaa { samples: 8 })
    .add_plugins(DefaultPlugins)
    .add_startup_system(render::generate_particles)
    .add_startup_system(render::generate_ui)
    .add_stage_after(
      CoreStage::Update,
      FixedUpdateStage,
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
