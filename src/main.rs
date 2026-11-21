use bevy::prelude::*;
use bevy::window::WindowMode;
#[cfg(feature = "brp")]
use bevy_brp_extras::BrpExtrasPlugin;
use clap::Parser;

use partikl::{args, gpu, loading, ui};

fn main() {
  let program_args = args::ProgramArgs::parse();
  let spec = loading::get_particle_spec(&program_args).unwrap_or_else(|error| {
    eprintln!("błąd: {error}");
    std::process::exit(1);
  });
  let mut app = App::new();
  app
    .insert_resource(spec)
    .insert_resource(program_args.sim_options())
    .insert_resource(program_args)
    .add_plugins(
      DefaultPlugins
        .set(WindowPlugin {
          primary_window: Some(Window {
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
    .add_plugins((gpu::GpuPlugin, ui::UiPlugin));
  #[cfg(feature = "brp")]
  app.add_plugins(BrpExtrasPlugin::default());
  app.run();
}
