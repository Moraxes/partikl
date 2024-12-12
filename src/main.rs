use core::{SimState, DELTA_TIME};

use bevy::prelude::*;
use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::window::{PresentMode, WindowMode, WindowResolution};
use clap::Parser;

mod args;
mod core;
mod loading;
mod render;
mod sim;
mod ui;

fn main() {
  let program_args = args::ProgramArgs::parse();
  App::new()
    .insert_resource(loading::get_particle_spec(&program_args))
    .insert_resource(program_args)
    .insert_resource(Time::<Fixed>::from_seconds(DELTA_TIME))
    .add_plugins(DefaultPlugins.set(WindowPlugin {
      primary_window: Some(Window {
        resolution: WindowResolution::new(2560.0, 1440.0),
        position: WindowPosition::Automatic,
        resize_constraints: Default::default(),
        title: "partikl".to_string(),
        present_mode: PresentMode::Mailbox,
        resizable: false,
        decorations: false,
        mode: WindowMode::BorderlessFullscreen(MonitorSelection::Primary),
        transparent: false,
        ..Default::default()
      }),
      ..Default::default()
    }))
    .add_plugins(FrameTimeDiagnosticsPlugin::default())
    .init_state::<SimState>()
    .add_systems(Startup, (
      render::init_materials,
      render::init_particles,
    ).chain())
    .add_systems(Startup, ui::init_ui)
    .add_systems(FixedUpdate, {
      use sim::*;
      (
        compute_forces.before(compute_friction),
        compute_friction.before(integrate),
        integrate,
        wrap_around.after(integrate),
        update_shape.after(integrate),
      )
    })
    .add_systems(Update, (
      sim::select_on_click,
      ui::update_text,
      ui::exit_after_time,
      ui::handle_keyboard_input,
      ui::handle_mouse_input,
      ui::close_on_esc,
    ))
    .run();
}
