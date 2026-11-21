//! Krok symulacji GPU: czas na krok (z synchronizacją z GPU) dla liczb cząstek `--sizes`,
//! po `--steps` kroków rozbiegu od stanu losowego, w wariantach `SimOptions`. Wyniki w Parquet
//! (kolumny: `n`, `variant`, `spec_seed`, `step_offset`, `rep`, `step_ns`), porównanie przed/po
//! przez `tools/bench-ab/ab.sh`. Mierzy cały krok od wywołania do końca pracy GPU, z narzutem
//! klatki Bevy (przy małym `n` dominuje); `cargo bench --bench gpu -- --help` pokazuje parametry.

#[path = "../common/mod.rs"]
mod common;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use arrow_array::{ArrayRef, RecordBatch, StringArray, UInt16Array, UInt32Array, UInt64Array};
use arrow_schema::{DataType, Field, Schema};
use clap::Parser;

use common::{Output, WORLD, median, nanos, particle_spec, rng};
use partikl::gpu::headless::Harness;
use partikl::gpu::initial_particles;
use partikl::options::{Overlap, SimOptions};

/// Macierz trybu `--quick`. Wybrana na CPU jako ta, w której gęstość prawie nie rośnie z
/// czasem (rzadki stan); stany o kontrolowanej gęstości: zadanie 052.
const QUICK_SEED: u64 = 23;
/// Mierzone pomiary w trybie `--quick` (bez niego domyślnie 10).
const QUICK_MEASURED: usize = 100;

#[derive(Parser, Debug)]
struct Args {
  /// Liczby cząstek.
  #[arg(long, value_delimiter = ',', default_value = "9000,100000")]
  sizes: Vec<usize>,
  /// Kroki rozbiegu przed pomiarem.
  #[arg(long, default_value_t = 512)]
  steps: u64,
  /// Kroków w jednym pomiarze (czas dzielony przez tę liczbę). Powyżej `MAX_STEPS_PER_FRAME`
  /// kroki idą w kilku klatkach.
  #[arg(long, default_value_t = 16, value_parser = clap::value_parser!(u64).range(1..))]
  batch: u64,
  /// Pomiar wszystkich wariantów `SimOptions` zamiast samego domyślnego.
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
  /// Ustawienia porównań przed/po (`ab.sh`): macierz `QUICK_SEED`, `QUICK_MEASURED` pomiarów.
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
  let schema = Arc::new(Schema::new(vec![
    Field::new("n", DataType::UInt32, false),
    Field::new("variant", DataType::Utf8, false),
    Field::new("spec_seed", DataType::UInt64, false),
    Field::new("step_offset", DataType::UInt32, false),
    Field::new("rep", DataType::UInt16, false),
    Field::new("step_ns", DataType::UInt64, false),
  ]));
  let mut output = Output::new("gpu", &args.out_dir, args.warmup, measured, schema.clone());
  let variants: Vec<SimOptions> = if args.all_variants {
    Overlap::ALL.iter().flat_map(|&overlap| [false, true].map(|average_forces| SimOptions { average_forces, overlap })).collect()
  } else {
    vec![SimOptions::default()]
  };
  println!("{:>9} {:<24} {:>12}", "n", "variant", "median µs/step");
  'sizes: for &n in &args.sizes {
    for &options in &variants {
      let spec = particle_spec(args.types, seed);
      let particles = initial_particles(n, args.types, WORLD, &mut rng(seed));
      let Some(mut harness) = Harness::new(options, spec, particles, WORLD) else {
        // Zapisane już wiersze trafiają do pliku (`finish` poniżej).
        break 'sizes;
      };
      harness.step(args.steps);
      let mut times = Vec::new();
      for _ in 0..args.warmup + measured {
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
          Arc::new(StringArray::from(vec![variant.clone(); rows])),
          Arc::new(UInt64Array::from(vec![seed; rows])),
          Arc::new(UInt32Array::from(vec![args.steps as u32; rows])),
          Arc::new(UInt16Array::from((0..rows as u16).collect::<Vec<_>>())),
          Arc::new(UInt64Array::from(samples.to_vec())),
        ],
      )
      .unwrap();
      output.write(&batch);
      let mut floats: Vec<f64> = samples.iter().map(|&t| t as f64).collect();
      println!("{n:>9} {variant:<24} {:>12.1}", median(&mut floats) / 1000.0);
    }
  }
  println!("wyniki: {}", output.path.display());
  output.finish();
}
