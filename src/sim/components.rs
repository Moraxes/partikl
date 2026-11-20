use bevy::prelude::*;

#[derive(Component, Default, Debug)]
pub struct Acceleration(pub Vec2);
#[derive(Component, Default, Debug)]
pub struct LastPosition(pub Vec2);

/// Typ cząstki: indeks wiersza/kolumny w macierzy sił z `ParticleSpec`.
#[derive(Component, Default, Clone, Copy)]
pub struct InteractionId(pub usize);

#[derive(Bundle, Default)]
pub struct ParticleBundle {
  pub last_pos: LastPosition,
  pub acceleration: Acceleration,
  pub interaction: InteractionId,
}
