//! Krok symulacji GPU: czas na krok (od wywołania do końca pracy GPU) dla liczb cząstek
//! `--sizes`, scen `--scenes` i wariantów `SimOptions`. Sceny syntetyczne (`scenes.rs`) mają
//! stałą gęstość: przed każdym pomiarem stan wraca do stanu sceny, więc każdy pomiar (i obie
//! strony porównania przed/po) liczy ten sam stan. Scena `evolved` to dawny pomiar: stan
//! losowy po `--steps` krokach rozbiegu, potem pomiary kolejno.
//!
//! Czas zawiera narzut klatki Bevy (przy małym `n` dominuje); czysty czas GPU: zadanie 047.
//! Wyniki w Parquet (kolumny: `n`, `scene`, `variant`, `spec_seed`, `step_offset`,
//! `candidates_mean`, `rep`, `step_ns`), porównanie przez `tools/bench-ab/ab.sh`;
//! `cargo bench --bench gpu -- --help` pokazuje parametry.

#[path = "../common/mod.rs"]
mod common;
mod scenes;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use arrow_array::{ArrayRef, Float32Array, RecordBatch, StringArray, UInt16Array, UInt32Array, UInt64Array};
use arrow_schema::{DataType, Field, Schema};
use clap::Parser;

use common::{Output, WORLD, median, nanos, particle_spec, rng};
use partikl::gpu::headless::Harness;
use partikl::options::{Overlap, SimOptions};
use scenes::{Scene, candidates_mean};

/// Macierz trybu `--quick`.
const QUICK_SEED: u64 = 23;
/// Mierzone pomiary w trybie `--quick` (bez niego domyślnie 10).
const QUICK_MEASURED: usize = 100;
/// Liczba cząstek w trybie `--quick`: optymalizujemy pod duże `n`.
const QUICK_N: usize = 100_000;

#[derive(Parser, Debug)]
struct Args {
  /// Liczby cząstek.
  #[arg(long, value_delimiter = ',', default_value = "9000,100000")]
  sizes: Vec<usize>,
  /// Sceny (gęstości).
  #[arg(long, value_delimiter = ',', default_value = "uniform,clusters,dense")]
  scenes: Vec<Scene>,
  /// Kroki rozbiegu przed pomiarem sceny `evolved`.
  #[arg(long, default_value_t = 512)]
  steps: u64,
  /// Kroków w jednym pomiarze (czas dzielony przez tę liczbę). Powyżej `MAX_STEPS_PER_FRAME`
  /// kroki idą w kilku klatkach.
  #[arg(long, default_value_t = 16, value_parser = clap::value_parser!(u64).range(1..))]
  batch: u64,
  /// Wszystkie warianty `SimOptions` zamiast domyślnego wariantu aplikacji (uśrednianie sił).
  #[arg(long)]
  all_variants: bool,
  /// Ziarno macierzy sił i rozmieszczenia.
  #[arg(long, default_value_t = 0)]
  seed: u64,
  /// Liczba typów cząstek.
  #[arg(long, default_value_t = 3)]
  types: usize,
  /// Niemierzone pomiary przed mierzonymi.
  #[arg(long, default_value_t = 3)]
  warmup: usize,
  /// Mierzone pomiary na przypadek; domyślnie 10, z `--quick` `QUICK_MEASURED`.
  #[arg(long)]
  measured: Option<usize>,
  /// Katalog wyników.
  #[arg(long, default_value = ".artifacts/bench")]
  out_dir: PathBuf,
  /// Ustawienia porównań przed/po (`ab.sh`): `QUICK_N` cząstek, macierz `QUICK_SEED`,
  /// `QUICK_MEASURED` pomiarów. Nadpisuje `--sizes` i `--seed`.
  #[arg(long)]
  quick: bool,
  /// Przekazywane przez `cargo bench`; ignorowane.
  #[arg(long, hide = true)]
  bench: bool,
}

fn main() {
  let args = Args::parse();
  let measured = args.measured.unwrap_or(if args.quick { QUICK_MEASURED } else { 10 });
  let seed = if args.quick { QUICK_SEED } else { args.seed };
  let sizes = if args.quick { vec![QUICK_N] } else { args.sizes.clone() };
  let schema = Arc::new(Schema::new(vec![
    Field::new("n", DataType::UInt32, false),
    Field::new("scene", DataType::Utf8, false),
    Field::new("variant", DataType::Utf8, false),
    Field::new("spec_seed", DataType::UInt64, false),
    Field::new("step_offset", DataType::UInt32, false),
    Field::new("candidates_mean", DataType::Float32, false),
    Field::new("rep", DataType::UInt16, false),
    Field::new("step_ns", DataType::UInt64, false),
  ]));
  let mut output = Output::new("gpu", &args.out_dir, args.warmup, measured, schema.clone());
  let variants: Vec<SimOptions> = if args.all_variants {
    Overlap::ALL.iter().flat_map(|&overlap| [false, true].map(|average_forces| SimOptions { average_forces, overlap })).collect()
  } else {
    vec![SimOptions { average_forces: true, overlap: Overlap::default() }]
  };
  println!("{:>9} {:<9} {:<24} {:>10} {:>12}", "n", "scene", "variant", "kandydaci", "median µs/step");
  'cases: for &n in &sizes {
    for &scene in &args.scenes {
      for &options in &variants {
        let spec = particle_spec(args.types, seed);
        let state = scene.particles(n, args.types, WORLD, &mut rng(seed));
        let Some(mut harness) = Harness::new(options, spec, state.clone(), WORLD) else {
          // Zapisane już wiersze trafiają do pliku (`finish` poniżej).
          break 'cases;
        };
        let (step_offset, candidates) = if scene == Scene::Evolved {
          harness.step(args.steps);
          (args.steps, candidates_mean(&harness.run(0), WORLD))
        } else {
          (0, candidates_mean(&state, WORLD))
        };
        let mut times = Vec::new();
        for _ in 0..args.warmup + measured {
          if scene != Scene::Evolved {
            harness.set_state(state.clone());
          }
          let start = Instant::now();
          harness.step(args.batch);
          times.push(nanos(start.elapsed()) / args.batch);
        }
        let samples = &times[args.warmup..];
        let variant = format!("{}{}", options.overlap, if options.average_forces { "+average" } else { "" });
        let rows = samples.len();
        let batch = RecordBatch::try_new(
          schema.clone(),
          vec![
            Arc::new(UInt32Array::from(vec![n as u32; rows])) as ArrayRef,
            Arc::new(StringArray::from(vec![scene.to_string(); rows])),
            Arc::new(StringArray::from(vec![variant.clone(); rows])),
            Arc::new(UInt64Array::from(vec![seed; rows])),
            Arc::new(UInt32Array::from(vec![step_offset as u32; rows])),
            Arc::new(Float32Array::from(vec![candidates as f32; rows])),
            Arc::new(UInt16Array::from((0..rows as u16).collect::<Vec<_>>())),
            Arc::new(UInt64Array::from(samples.to_vec())),
          ],
        )
        .unwrap();
        output.write(&batch);
        let mut floats: Vec<f64> = samples.iter().map(|&t| t as f64).collect();
        println!("{n:>9} {scene:<9} {variant:<24} {candidates:>10.0} {:>12.1}", median(&mut floats) / 1000.0);
      }
    }
  }
  println!("wyniki: {}", output.path.display());
  output.finish();
}
