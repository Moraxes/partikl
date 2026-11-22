//! Rozmiar ekranu, na którym działa symulacja.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::{Monitor, PrimaryMonitor, PrimaryWindow};

/// Rozmiar świata symulacji w jednostkach logicznych: ekran głównego monitora, bo okno jest
/// na pełnym ekranie. `Window` w `Startup` ma jeszcze rozmiar sprzed przejścia na pełny
/// ekran (prawdziwy przychodzi z pierwszym zdarzeniem resize), więc czytamy monitor. Bez
/// monitora (testy) rozmiar okna.
#[derive(SystemParam)]
pub struct ScreenSize<'w, 's> {
  monitors: Query<'w, 's, &'static Monitor, With<PrimaryMonitor>>,
  windows: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
}

impl ScreenSize<'_, '_> {
  pub fn get(&self) -> Vec2 {
    if let Ok(monitor) = self.monitors.single() {
      return UVec2::new(monitor.physical_width, monitor.physical_height).as_vec2()
        / monitor.scale_factor as f32;
    }
    let window = self.windows.single().expect("no primary window");
    Vec2::new(window.width(), window.height())
  }
}
