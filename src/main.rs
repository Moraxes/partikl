use bevy::prelude::*;
use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::ecs::schedule::ShouldRun;
use bevy::window::{close_on_esc, PresentMode, WindowMode, WindowResolution};
use structopt::StructOpt;

mod args;
mod core;
mod loading;
mod render;
mod sim;
mod ui;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[derive(StageLabel)]
enum Stage {
  InitCategories,
  FixedUpdateStage,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[derive(SystemLabel)]
enum System {
  InitParticles,
}

fn main() {
  let program_args = args::ProgramArgs::from_args();
  App::new()
    .insert_resource(loading::get_particle_spec(&program_args))
    .insert_resource(program_args)
    .insert_resource(Msaa::Sample4)
    .add_plugins(DefaultPlugins.set(WindowPlugin {
      primary_window: Some(Window {
        resolution: WindowResolution::new(1920.0, 1080.0),
        position: WindowPosition::Automatic,
        resize_constraints: Default::default(),
        title: "partikl".to_string(),
        present_mode: PresentMode::Mailbox,
        resizable: false,
        decorations: false,
        mode: WindowMode::BorderlessFullscreen,
        transparent: false,
        ..Default::default()
      }),
      ..Default::default()
    }))
    .add_plugin(FrameTimeDiagnosticsPlugin::default())
    .add_stage_before(
      CoreStage::First,
      Stage::InitCategories,
      SystemStage::single_threaded()
        .with_run_criteria(ShouldRun::once)
        .with_system(render::init_materials))
    .add_startup_system(render::init_particles.label(System::InitParticles))
    .add_startup_system(ui::init_ui)
    .add_stage_after(
      CoreStage::First,
      Stage::FixedUpdateStage,
      sim::simulation_stage(),
    )
    .add_system(ui::update_text)
    .add_system(ui::exit_after_time)
    .add_system(ui::handle_keyboard_input)
    .add_system(ui::handle_mouse_input)
    .add_system(close_on_esc)
    .run();
}
