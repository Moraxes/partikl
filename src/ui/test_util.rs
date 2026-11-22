use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy::window::{CursorOptions, PrimaryWindow};

use crate::gpu::{RenderQuality, SamplingCap, Substeps};
use crate::options::{RuleSchema, SimOptions, SimState};

/// Aplikacja bez okna i bez `InputPlugin`: `ButtonInput` nie jest czyszczony między
/// klatkami, więc `just_pressed` zostaje do ręcznego `clear_input`.
pub fn input_app() -> App {
  let mut app = App::new();
  app
    .add_plugins((MinimalPlugins, StatesPlugin))
    .init_state::<SimState>()
    .init_resource::<SimOptions>()
    .insert_resource(builtin_schema())
    .init_resource::<RenderQuality>()
    .init_resource::<SamplingCap>()
    .init_resource::<Substeps>()
    .init_resource::<ButtonInput<KeyCode>>()
    .init_resource::<ButtonInput<MouseButton>>()
    .add_message::<MouseWheel>()
    .add_message::<MouseMotion>();
  app
    .world_mut()
    .spawn((Window::default(), PrimaryWindow, CursorOptions::default()));
  app
}

/// Kończy "klatkę" wejścia: zeruje `just_pressed`/`just_released`.
pub fn clear_input(app: &mut App) {
  app
    .world_mut()
    .resource_mut::<ButtonInput<KeyCode>>()
    .clear();
  app
    .world_mut()
    .resource_mut::<ButtonInput<MouseButton>>()
    .clear();
}

/// Opcje wbudowanych reguł (jak w aplikacji domyślnie).
pub fn builtin_schema() -> RuleSchema {
  crate::gpu::RuleSource::Builtin
    .read()
    .expect("wbudowane reguły")
    .schema
}
