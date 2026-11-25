//! Losowanie w trakcie działania: nowa macierz sił (klawisz R) i nowa paleta kolorów typów
//! (klawisz Y), do których symulacja przechodzi płynnie w czasie `TRANSITION`, oraz od razu nowy
//! stan cząstek z nową paletą (klawisz T).

use std::path::{Path, PathBuf};
use std::time::Duration;

use bevy::color::{Mix, Oklaba};
use bevy::prelude::*;
use bevy::render::storage::ShaderBuffer;

use super::{
  GpuSim, Palette, color_buffer, initial_particles, model_buffer, random_colors, state_buffer,
};
use crate::args::ProgramArgs;
use crate::loading::{LoadError, dump_timestamped_spec, generate_particle_spec};
use crate::options::SimState;
use crate::spec::{Interaction, ParticleSpec};

/// Czas przejścia do nowej macierzy albo palety (czas wirtualny).
const TRANSITION: Duration = Duration::from_secs(1);

/// Prośba o nową macierz sił.
#[derive(Message, Clone, Copy, Debug)]
pub struct RerollMatrix;

/// Prośba o nowy stan cząstek i kolory typów.
#[derive(Message, Clone, Copy, Debug)]
pub struct RerollState;

/// Prośba o nowe kolory typów.
#[derive(Message, Clone, Copy, Debug)]
pub struct RerollColors;

/// Trwające przejście macierzy (w pauzie stoi, jak symulacja).
#[derive(Resource, Default)]
struct MatrixTransition(Option<Tween<Interaction>>);

/// Trwające przejście palety (tylko wygląd, więc biegnie też w pauzie).
#[derive(Resource, Default)]
struct ColorTransition(Option<Tween<LinearRgba>>);

/// Przejście od listy wartości `from` do `to` (wierszy macierzy, kolorów typów).
struct Tween<T> {
  from: Vec<T>,
  to: Vec<T>,
  elapsed: Duration,
}

impl<T> Tween<T> {
  fn new(from: Vec<T>, to: Vec<T>) -> Self {
    Tween {
      from,
      to,
      elapsed: Duration::ZERO,
    }
  }
}

/// Przesuwa trwające przejście o `delta` i zwraca bieżące wartości (`mix` daje wartość po części
/// `t` z 0..1 przejścia); po czasie `TRANSITION` zwraca dokładnie cel i kończy przejście. `None`,
/// gdy nic nie trwa.
fn advance<T>(
  transition: &mut Option<Tween<T>>,
  delta: Duration,
  mix: impl Fn(&T, &T, f32) -> T,
) -> Option<Vec<T>> {
  let mut tween = transition.take()?;
  tween.elapsed += delta;
  if tween.elapsed >= TRANSITION {
    return Some(tween.to);
  }
  let t = tween.elapsed.div_duration_f32(TRANSITION);
  let values = tween
    .from
    .iter()
    .zip(&tween.to)
    .map(|(from, to)| mix(from, to, t))
    .collect();
  *transition = Some(tween);
  Some(values)
}

/// Wiersz macierzy w trakcie przejścia: każdy współczynnik liniowo.
fn mix_row(from: &Interaction, to: &Interaction, t: f32) -> Interaction {
  Interaction {
    force_coeffs: from
      .force_coeffs
      .iter()
      .zip(&to.force_coeffs)
      .map(|(a, b)| a + (b - a) * t)
      .collect(),
  }
}

/// Kolor w trakcie przejścia: liniowo w Oklab, więc jasność zmienia się równo dla oka.
fn mix_color(from: &LinearRgba, to: &LinearRgba, t: f32) -> LinearRgba {
  Oklaba::from(*from).mix(&Oklaba::from(*to), t).into()
}

pub(super) struct ResetPlugin;

impl Plugin for ResetPlugin {
  fn build(&self, app: &mut App) {
    app
      .add_message::<RerollMatrix>()
      .add_message::<RerollState>()
      .add_message::<RerollColors>()
      .init_resource::<MatrixTransition>()
      .init_resource::<ColorTransition>()
      .add_systems(
        Update,
        (
          // Najpierw krok trwającego przejścia: ponowne losowanie startuje z macierzy tej klatki.
          (
            advance_matrix_transition.run_if(in_state(SimState::Running)),
            start_matrix_transition.run_if(on_message::<RerollMatrix>),
          )
            .chain(),
          // Jak wyżej dla palety; nowy stan kończy jej przejście, a nowe przejście startuje z
          // palety nowego stanu. W `Update`, przed zdarzeniami assetów (`PostUpdate`): nowe
          // bufory i `state_epoch` trafiają do świata renderowania w tej samej klatce.
          (
            advance_color_transition.run_if(resource_exists::<Palette>),
            reroll_state
              .run_if(resource_exists::<GpuSim>)
              .run_if(on_message::<RerollState>),
            start_color_transition
              .run_if(resource_exists::<Palette>)
              .run_if(on_message::<RerollColors>),
            upload_palette
              .run_if(resource_exists::<GpuSim>)
              .run_if(resource_exists_and_changed::<Palette>),
          )
            .chain(),
        ),
      );
  }
}

/// Przesuwa macierz w stronę celu. Bez przejścia nie dotyka `ParticleSpec`, więc macierz nie
/// jest wysyłana na GPU.
fn advance_matrix_transition(
  time: Res<Time>,
  mut transition: ResMut<MatrixTransition>,
  mut spec: ResMut<ParticleSpec>,
) {
  if let Some(interactions) = advance(&mut transition.0, time.delta(), mix_row) {
    spec.interactions = interactions;
  }
}

/// Losuje macierz docelową, od razu ją zapisuje i zaczyna przejście od bieżącej.
fn start_matrix_transition(
  args: Option<Res<ProgramArgs>>,
  spec: Res<ParticleSpec>,
  mut transition: ResMut<MatrixTransition>,
) {
  let target = ParticleSpec {
    params: spec.params,
    ..generate_particle_spec(&mut rand::rng(), spec.interactions.len())
  };
  match dump_rolled(Path::new(""), args.as_deref(), &target) {
    Ok(Some(path)) => info!("nowa macierz zapisana w {}", path.display()),
    Ok(None) => {}
    Err(error) => error!("{error}"),
  }
  transition.0 = Some(Tween::new(spec.interactions.clone(), target.interactions));
}

/// Zapisuje wylosowaną specyfikację w `dir`, chyba że `--no-dump-interaction-spec` (bez
/// argumentów programu, jak w testach, też nic nie zapisuje).
fn dump_rolled(
  dir: &Path,
  args: Option<&ProgramArgs>,
  spec: &ParticleSpec,
) -> Result<Option<PathBuf>, LoadError> {
  if args.is_none_or(|args| args.no_dump_interaction_spec) {
    return Ok(None);
  }
  dump_timestamped_spec(dir, spec).map(Some)
}

/// Przesuwa paletę w stronę celu. Bez przejścia nie dotyka `Palette`.
fn advance_color_transition(
  time: Res<Time>,
  mut transition: ResMut<ColorTransition>,
  mut palette: ResMut<Palette>,
) {
  if let Some(colors) = advance(&mut transition.0, time.delta(), mix_color) {
    palette.0 = colors;
  }
}

/// Losuje paletę docelową i zaczyna przejście od bieżącej.
fn start_color_transition(palette: Res<Palette>, mut transition: ResMut<ColorTransition>) {
  let target = Palette::new(random_colors(palette.0.len(), &mut rand::rng()));
  transition.0 = Some(Tween::new(palette.0.clone(), target.0));
}

/// Wysyła zmienioną paletę do bufora kolorów (ten sam rozmiar, więc materiał zostaje ten sam).
fn upload_palette(
  palette: Res<Palette>,
  sim: Res<GpuSim>,
  mut buffers: ResMut<Assets<ShaderBuffer>>,
) {
  if let Err(error) = buffers.insert(&sim.colors, color_buffer(&palette.0)) {
    error!("nie da się podmienić bufora kolorów: {error}");
  }
}

/// Podmienia stan cząstek i kolory typów na nowe losowe (od razu, kończąc przejście palety),
/// zeruje model.
fn reroll_state(
  mut sim: ResMut<GpuSim>,
  mut buffers: ResMut<Assets<ShaderBuffer>>,
  mut palette: ResMut<Palette>,
  mut transition: ResMut<ColorTransition>,
) {
  let n = sim.num_particles as usize;
  let types = sim.num_types as usize;
  let mut rng = rand::rng();
  let particles = initial_particles(n, types, sim.geometry.world, &mut rng);
  *palette = Palette::new(random_colors(types, &mut rng));
  transition.0 = None;
  let mut writes = vec![(&sim.state, state_buffer(particles))];
  if sim.model_stride > 0 {
    writes.push((&sim.model, model_buffer(n, sim.model_stride)));
  }
  for (handle, buffer) in writes {
    if let Err(error) = buffers.insert(handle, buffer) {
      error!("nie da się podmienić bufora cząstek: {error}");
    }
  }
  sim.state_epoch += 1;
}

#[cfg(test)]
mod tests;
