//! Sonda stabilności: symulacja z macierzą sił z pliku `.ron` (np. zapisanego przez program)
//! bez okna i co `--every` kroków statystyki ruchu: mediana, p99 i maksimum prędkości, odsetek
//! cząstek szybszych niż `--fast` oraz najpełniejsza komórka siatki (miara zagęszczenia
//! skupisk). Do sprawdzania, czy dana macierz przy dużym `n` się „rozsypuje” i jak pomagają
//! opcje symulacji.
//!
//! ```text
//! cargo bench --bench probe -- spec.ron
//! cargo bench --bench probe -- spec.ron --n 100000 --steps 1500 --average --overlap adaptive-range
//! cargo bench --bench probe -- spec.ron --positions stan.json
//! ```
//!
//! Rozmieszczenie jak w benchu `sim` (losowe pozycje i prędkości z ziarna `--seed`), świat
//! jak domyślne okno. `--positions` zapisuje końcowe pozycje, typy i prędkości jako JSON
//! `[{seed, particles: [[x, y, typ, prędkość], …]}]`.

use std::path::PathBuf;
use std::time::Instant;

use bevy::app::TaskPoolPlugin;
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use clap::Parser;
use rand::prelude::*;

use partikl::config::{DELTA_TIME, INTERACTION_RADIUS};
use partikl::sim::*;
use partikl::spec::{Interaction, ParticleSpec};

/// Rozmiar świata, jak domyślne okno w `main.rs`.
const WORLD: Vec2 = Vec2::new(2560.0, 1440.0);

#[derive(Parser)]
struct Args {
  /// Macierz sił (`.ron`, jak pliki `spec-*.ron` zapisywane przez program).
  spec: PathBuf,
  /// Liczba cząstek.
  #[arg(long, default_value_t = 100_000)]
  n: usize,
  /// Liczba kroków symulacji.
  #[arg(long, default_value_t = 1500)]
  steps: usize,
  /// Co ile kroków wypisać statystyki (i zawsze po ostatnim).
  #[arg(long, default_value_t = 250)]
  every: usize,
  /// Próg „szybkiej” cząstki (jednostki świata na sekundę).
  #[arg(long, default_value_t = 1000.0)]
  fast: f32,
  /// Ziarno rozmieszczenia.
  #[arg(long, default_value_t = 1)]
  seed: u64,
  /// Uśrednianie sił (`SimOptions::average_forces`).
  #[arg(long)]
  average: bool,
  /// Tryb nakładania (`SimOptions::overlap`).
  #[arg(long, value_enum, default_value_t = Overlap::Spring)]
  overlap: Overlap,
  /// Plik JSON na końcowy stan.
  #[arg(long)]
  positions: Option<PathBuf>,
}

fn main() {
  // `cargo bench` dokłada `--bench`.
  let args = Args::parse_from(std::env::args().filter(|a| a != "--bench"));
  let file = std::fs::File::open(&args.spec).unwrap_or_else(|e| panic!("{}: {e}", args.spec.display()));
  let spec: Vec<Interaction> = ron::de::from_reader(file).unwrap_or_else(|e| panic!("{}: {e}", args.spec.display()));
  let types = spec.len();
  let options = SimOptions { average_forces: args.average, overlap: args.overlap };
  eprintln!("n = {}, {} typów, {options:?}", args.n, types);

  let mut app = App::new();
  app
    .add_plugins((MinimalPlugins.set(TaskPoolPlugin { task_pool_options: task_pool_options() }), StatesPlugin, SimPlugin))
    .insert_resource(ParticleSpec { interactions: spec });
  let mut rng = StdRng::seed_from_u64(args.seed);
  let mut grid = SpatialGrid::new(WORLD.x, WORLD.y, INTERACTION_RADIUS);
  let mut placed = Vec::with_capacity(args.n);
  for _ in 0..args.n {
    let position = Vec2::new(rng.random_range(-WORLD.x / 2.0..WORLD.x / 2.0), rng.random_range(-WORLD.y / 2.0..WORLD.y / 2.0));
    let velocity = Vec2::new(rng.random_range(-125.0..125.0), rng.random_range(-125.0..125.0));
    let interaction = InteractionId(rng.random_range(0..types));
    let entity = app
      .world_mut()
      .spawn((
        ParticleBundle {
          last_pos: LastPosition(position - DELTA_TIME as f32 * velocity),
          acceleration: Acceleration(Vec2::ZERO),
          interaction,
        },
        Transform::from_translation(position.extend(0.0)),
      ))
      .id();
    placed.push((entity, position, interaction));
  }
  grid.rebuild(placed);
  app.insert_resource(grid);
  app.finish();
  app.cleanup();

  // Systemy w kolejności `SimSet`, bez `update_shape` (tylko wygląd).
  let world = app.world_mut();
  let systems = [
    register_forces(world, options),
    world.register_system(compute_friction),
    world.register_system(integrate),
    world.register_system(wrap_around),
    world.register_system(rebuild_grid),
  ];
  let start = Instant::now();
  for step in 1..=args.steps {
    for id in systems {
      app.world_mut().run_system(id).unwrap();
    }
    if step % args.every.max(1) == 0 || step == args.steps {
      report(app.world_mut(), step, args.fast, start);
    }
  }

  if let Some(path) = &args.positions {
    let world = app.world_mut();
    let particles: Vec<[f32; 4]> = world
      .query::<(&Transform, &InteractionId, &LastPosition)>()
      .iter(world)
      .map(|(t, i, l)| [t.translation.x, t.translation.y, i.0 as f32, speed(t, l)])
      .collect();
    let state = serde_json::json!([{ "seed": args.seed, "particles": particles }]);
    std::fs::write(path, serde_json::to_vec(&state).unwrap()).unwrap();
    eprintln!("stan: {}", path.display());
  }
}

fn speed(transform: &Transform, last: &LastPosition) -> f32 {
  (transform.translation.truncate() - last.0).length() / DELTA_TIME as f32
}

fn report(world: &mut World, step: usize, fast: f32, start: Instant) {
  let mut speeds: Vec<f32> = world.query::<(&Transform, &LastPosition)>().iter(world).map(|(t, l)| speed(t, l)).collect();
  if speeds.is_empty() {
    return;
  }
  speeds.sort_by(f32::total_cmp);
  let quantile = |q: f64| speeds[((speeds.len() - 1) as f64 * q) as usize];
  let fast_share = speeds.iter().filter(|&&s| s > fast).count() as f64 / speeds.len() as f64;
  let grid = world.resource::<SpatialGrid>();
  let fullest = (0..grid.cell_count()).map(|c| grid.cell_range(c).len()).max().unwrap_or(0);
  println!(
    "krok {step:5}: prędkość med {:7.1} p99 {:8.1} max {:9.1}, >{fast} j/s {:5.2}%, najpełniejsza komórka {fullest:5}, {:.1} s",
    quantile(0.5),
    quantile(0.99),
    quantile(1.0),
    100.0 * fast_share,
    start.elapsed().as_secs_f64()
  );
}
