//! Gorąca ścieżka symulacji: czas kroku i każdego systemu w stanach po `step_offset` krokach,
//! dla wielu macierzy sił. Wyniki: Parquet w `.artifacts/bench/` (format w
//! `common::results`), postęp w pliku
//! `.progress.json` obok, podsumowanie na stdout.
//!
//! `cargo bench --bench sim -- --help` pokazuje parametry.

#[path = "../common/mod.rs"]
mod common;

use std::collections::BTreeMap;

use common::results::{SampleKey, SimRows};
use common::{BenchArgs, Output, Progress, SYSTEM_NAMES, Sim, median, nanos, quantile};

/// Mediany jednej macierzy w jednym punkcie `(n, offset)`.
struct MatrixResult {
  step: f64,
  candidates: f64,
  systems: Vec<f64>,
}

const COLUMNS: [&str; 12] = [
  "n", "offset", "p10 µs", "med µs", "p90 µs", "kand med", "kand p90", "forces", "friction",
  "integrate", "wrap", "shape",
];

/// Rozkład po macierzach: kwantyle median kroku, kandydaci i mediana udziału każdego systemu.
fn summary_rows(summary: &BTreeMap<(usize, usize), Vec<MatrixResult>>) -> Vec<Vec<String>> {
  summary
    .iter()
    .map(|((n, offset), results)| {
      let mut steps: Vec<f64> = results.iter().map(|r| r.step).collect();
      let mut cands: Vec<f64> = results.iter().map(|r| r.candidates).collect();
      let mut row = vec![
        n.to_string(),
        offset.to_string(),
        format!("{:.0}", quantile(&mut steps, 0.1) / 1e3),
        format!("{:.0}", median(&mut steps) / 1e3),
        format!("{:.0}", quantile(&mut steps, 0.9) / 1e3),
        format!("{:.1}", median(&mut cands)),
        format!("{:.1}", quantile(&mut cands, 0.9)),
      ];
      for i in 0..SYSTEM_NAMES.len() {
        let mut shares: Vec<f64> =
          results.iter().map(|r| r.systems[i] / r.systems.iter().sum::<f64>()).collect();
        row.push(format!("{:.0}%", 100.0 * median(&mut shares)));
      }
      row
    })
    .collect()
}

fn to_f64(values: &[u64]) -> Vec<f64> {
  values.iter().map(|&x| x as f64).collect()
}

fn main() {
  let args = BenchArgs::parse_sorted();
  let mut output = Output::new("sim", &args, SimRows::schema());
  let mut rows = SimRows::default();
  let groups = args.n.iter().map(|&n| (format!("n={n}"), n as f64, args.seeds().len())).collect();
  let mut progress = Progress::new(&output, groups, &COLUMNS);
  let mut summary: BTreeMap<(usize, usize), Vec<MatrixResult>> = BTreeMap::new();

  for sample in args.samples() {
    let group = args.n.iter().position(|&n| n == sample.n).unwrap();
    progress.begin(&format!("n={} spec_seed={}", sample.n, sample.spec_seed));
    let mut sim = Sim::new(&sample);
    let mut steps_done = 0;
    for &offset in &args.offsets {
      while steps_done < offset {
        sim.step();
        steps_done += 1;
      }
      for _ in 0..args.warmup {
        sim.step();
      }
      let mut step_ns = vec![];
      let mut system_ns = vec![vec![]; SYSTEM_NAMES.len()];
      let mut candidates = vec![];
      for _ in 0..args.measured {
        // Kandydaci dla stanu, który przetwarza mierzony krok (poza pomiarem).
        candidates.push(sim.candidates_mean());
        let (step, per_system) = sim.step_timed();
        step_ns.push(nanos(step));
        for (i, d) in per_system.iter().enumerate() {
          system_ns[i].push(nanos(*d));
        }
      }
      steps_done += args.warmup + args.measured;

      let candidates_mean = candidates.iter().sum::<f64>() / candidates.len() as f64;
      let key = SampleKey::new(&sample, offset, candidates_mean);
      for rep in 0..args.measured {
        let systems = std::array::from_fn(|i| system_ns[i][rep]);
        rows.push(&key, rep, candidates[rep], step_ns[rep], systems);
      }

      summary.entry((sample.n, offset)).or_default().push(MatrixResult {
        step: median(&mut to_f64(&step_ns)),
        candidates: candidates_mean,
        systems: system_ns.iter().map(|ns| median(&mut to_f64(ns))).collect(),
      });
    }
    output.write(&rows.take_batch());
    progress.finish_sample(group, summary_rows(&summary));
  }
  progress.finish();

  println!(
    "\nKrok symulacji: rozkład median po macierzach ({} na n); kandydaci na sąsiada; \
     mediana udziału systemów w kroku",
    args.seeds().len()
  );
  println!("{}", common::format_table(&COLUMNS, &summary_rows(&summary)));
  println!("\nWyniki: {}", output.path.display());
  output.finish();
}
