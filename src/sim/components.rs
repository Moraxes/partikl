use bevy::prelude::*;

#[derive(Component, Default, Debug)]
#[require(Crowding)]
pub struct Acceleration(pub Vec2);

/// Wygładzona liczba sąsiadów w zasięgu (`N̄ ← N̄ + α(N − N̄)`), z której tryb
/// `Overlap::AdaptiveRange` liczy zasięg odpychania. Aktualizowana tylko w tym trybie.
#[derive(Component, Default, Debug)]
pub struct Crowding(pub f32);
#[derive(Component, Default, Debug)]
pub struct LastPosition(pub Vec2);

/// Typ cząstki: indeks wiersza/kolumny w macierzy sił z `ParticleSpec`.
#[derive(Component, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct InteractionId(pub usize);

#[derive(Bundle, Default)]
pub struct ParticleBundle {
  pub last_pos: LastPosition,
  pub acceleration: Acceleration,
  pub interaction: InteractionId,
}
