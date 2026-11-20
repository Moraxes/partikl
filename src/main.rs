use bevy::prelude::*;
use bevy::window::{WindowMode, WindowResolution};
use clap::Parser;

mod args;
mod config;
mod loading;
mod render;
mod sim;
mod spec;
mod ui;

fn main() {
  let program_args = args::ProgramArgs::parse();
  App::new()
    .insert_resource(loading::get_particle_spec(&program_args))
    .insert_resource(program_args)
    .add_plugins(DefaultPlugins.set(WindowPlugin {
      primary_window: Some(Window {
        resolution: WindowResolution::new(2560, 1440),
        position: WindowPosition::Automatic,
        resize_constraints: Default::default(),
        title: "partikl".to_string(),
        resizable: false,
        decorations: false,
        mode: WindowMode::BorderlessFullscreen(MonitorSelection::Primary),
        transparent: false,
        ..Default::default()
      }),
      ..Default::default()
    }))
    .add_plugins((sim::SimPlugin, render::RenderPlugin, ui::UiPlugin))
    .run();
}
