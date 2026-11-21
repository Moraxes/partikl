use bevy::app::TaskPoolPlugin;
use bevy::prelude::*;
use bevy::window::{WindowMode, WindowResolution};
use clap::Parser;

use partikl::{args, gpu, loading, render, sim, ui};

fn main() {
  let program_args = args::ProgramArgs::parse();
  let use_cpu = program_args.cpu;
  let mut app = App::new();
  app
    .insert_resource(loading::get_particle_spec(&program_args))
    .insert_resource(program_args.sim_options())
    .insert_resource(program_args)
    .add_plugins(
      DefaultPlugins
        .set(TaskPoolPlugin { task_pool_options: sim::task_pool_options() })
        .set(WindowPlugin {
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
        }),
    )
    ;
  if use_cpu {
    // Ścieżka legacy (silnik CPU sprzed portu na GPU).
    app.add_plugins((sim::SimPlugin, render::RenderPlugin, ui::UiPlugin));
  } else {
    app.add_plugins((gpu::GpuPlugin, ui::UiPlugin));
  }
  app.run();
}
