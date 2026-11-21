//! Krok symulacji GPU: czas na krok (z synchronizacją z GPU) dla liczb cząstek `--sizes`,
//! po `--steps` kroków rozbiegu, w wariantach `SimOptions`. Wyniki w Parquet jak pozostałe
//! benchmarki (kolumny: `n`, `variant`, `spec_seed`, `step_offset`, `rep`, `step_ns`), więc
//! `AB_BASE` i `tools/` działają tak samo. Mierzy cały krok (siatka + siły), nie przebiegi
//! osobno; `cargo bench --bench gpu -- --help` pokazuje parametry.

#[path = "../common/mod.rs"]
mod common;

use std::sync::Arc;
use std::time::Instant;

use arrow_array::{ArrayRef, RecordBatch, StringArray, UInt16Array, UInt32Array, UInt64Array};
use arrow_schema::{DataType, Field, Schema};
use clap::Parser;

use common::{BenchArgs, Output, WORLD, median, nanos, particle_spec, rng};
use partikl::gpu::headless::Harness;
use partikl::gpu::initial_particles;
use partikl::options::{Overlap, SimOptions};

#[derive(Parser, Debug)]
struct GpuArgs {
  #[command(flatten)]
  common: BenchArgs,
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
}

fn main() {
  let mut args = GpuArgs::parse();
  args.common.normalize();
  let schema = Arc::new(Schema::new(vec![
    Field::new("n", DataType::UInt32, false),
    Field::new("variant", DataType::Utf8, false),
    Field::new("spec_seed", DataType::UInt64, false),
    Field::new("step_offset", DataType::UInt32, false),
    Field::new("rep", DataType::UInt16, false),
    Field::new("step_ns", DataType::UInt64, false),
  ]));
  let mut output = Output::new("gpu", &args.common, schema.clone());
  let variants: Vec<SimOptions> = if args.all_variants {
    Overlap::ALL.iter().flat_map(|&overlap| [false, true].map(|average_forces| SimOptions { average_forces, overlap })).collect()
  } else {
    vec![SimOptions::default()]
  };
  let seed = if args.common.quick { common::QUICK_SEEDS[0] } else { args.common.seed_base };
  println!("{:>9} {:<24} {:>12}", "n", "variant", "median µs/step");
  'sizes: for &n in &args.sizes {
    for &options in &variants {
      let spec = particle_spec(args.common.types, seed);
      let particles = initial_particles(n, args.common.types, WORLD, &mut rng(seed));
      let Some(mut harness) = Harness::new(options, spec, particles, WORLD) else {
        // Zapisane już wiersze trafiają do pliku (`finish` poniżej).
        break 'sizes;
      };
      harness.step(args.steps);
      let mut times = Vec::new();
      for _ in 0..args.common.warmup + args.common.measured {
        let start = Instant::now();
        harness.step(args.batch);
        times.push(nanos(start.elapsed()) / args.batch);
      }
      let measured = &times[args.common.warmup..];
      let variant = format!("{}{}", options.overlap, if options.average_forces { "+average" } else { "" });
      let rows = measured.len();
      let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
          Arc::new(UInt32Array::from(vec![n as u32; rows])) as ArrayRef,
          Arc::new(StringArray::from(vec![variant.clone(); rows])),
          Arc::new(UInt64Array::from(vec![seed; rows])),
          Arc::new(UInt32Array::from(vec![args.steps as u32; rows])),
          Arc::new(UInt16Array::from((0..rows as u16).collect::<Vec<_>>())),
          Arc::new(UInt64Array::from(measured.to_vec())),
        ],
      )
      .unwrap();
      output.write(&batch);
      let mut floats: Vec<f64> = measured.iter().map(|&t| t as f64).collect();
      println!("{n:>9} {variant:<24} {:>12.1}", median(&mut floats) / 1000.0);
    }
  }
  println!("wyniki: {}", output.path.display());
  output.finish();
}
