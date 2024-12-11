use core::{SimState, DELTA_TIME};

use bevy::prelude::*;
use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::window::{close_on_esc, PresentMode, WindowMode, WindowResolution};
use structopt::StructOpt;

mod args;
mod core;
mod loading;
mod render;
mod sim;
mod ui;

fn main() {
  let program_args = args::ProgramArgs::from_args();
  App::new()
    .insert_resource(loading::get_particle_spec(&program_args))
    .insert_resource(program_args)
    .insert_resource(Msaa::Sample4)
    .insert_resource(FixedTime::new_from_secs(DELTA_TIME as f32))
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
    .add_state::<SimState>()
    .add_startup_systems((
      render::init_materials,
      render::init_particles,
    ).chain())
    .add_startup_system(ui::init_ui)
    .add_systems({
      use sim::*;
      (
        compute_forces.before(compute_friction),
        compute_friction.before(integrate),
        integrate,
        wrap_around.after(integrate),
        update_shape.after(integrate),
      )
        .in_schedule(CoreSchedule::FixedUpdate)
    })
    .add_system(sim::select_on_click)
    .add_system(ui::update_text)
    .add_system(ui::exit_after_time)
    .add_system(ui::handle_keyboard_input)
    .add_system(ui::handle_mouse_input)
    .add_system(close_on_esc)
    .run();
}
