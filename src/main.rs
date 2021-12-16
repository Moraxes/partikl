use bevy::prelude::*;
use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::ecs::schedule::RunOnce;
use bevy::window::WindowMode;
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
    .insert_resource(ClearColor(Color::rgb(0.0, 0.0, 0.0)))
    .insert_resource(Msaa { samples: 8 })
    .insert_resource(WindowDescriptor {
      width: 1920.0,
      height: 1080.0,
      position: None,
      resize_constraints: Default::default(),
      scale_factor_override: None,
      title: "partikl".to_string(),
      vsync: true,
      resizable: false,
      decorations: false,
      cursor_visible: true,
      cursor_locked: false,
      mode: WindowMode::BorderlessFullscreen
    })
    .add_plugins(DefaultPlugins)
    .add_plugin(FrameTimeDiagnosticsPlugin::default())
    .add_state(core::SimState::Running)
    .add_stage_before(
      CoreStage::Startup,
      Stage::InitCategories,
      SystemStage::single_threaded()
        .with_run_criteria(RunOnce::default())
        .with_system(render::init_appearances))
    .add_startup_system(render::init_particles.label(System::InitParticles))
    .add_startup_system(ui::init_ui)
    .add_stage_after(
      CoreStage::Startup,
      Stage::FixedUpdateStage,
      sim::simulation_stage(),
    )
    .add_system(ui::update_text)
    .add_system(ui::exit_after_time)
    .add_system(ui::handle_keyboard_input)
    .add_system(ui::handle_mouse_input)
    .add_system(bevy::input::system::exit_on_esc_system)
    .run();
}
