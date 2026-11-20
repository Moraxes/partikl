//! Zapis wyników benchmarków jako Parquet (zstd).
//!
//! Plik to jeden przebieg benchmarku. Układ długi: wiersz na jeden pomiar (`rep`) w próbce
//! `(n, macierz, step_offset)`, kolumny z nazwami zapisanymi raz w schemacie. Dane przebiegu
//! są w metadanych pliku (klucz-wartość w stopce): `schema`, `schema_version`, `bench`,
//! `revision`, `started`, `warmup`, `measured`, `world` (JSON `[x, y]`).
//!
//! Zgodność wsteczna (`SCHEMA_VERSION`, `major.minor`):
//! - minor: tylko nowe kolumny (czytniki biorą kolumny po nazwie, brakująca = pusta);
//! - major: zmiana nazwy, usunięcie albo zmiana znaczenia kolumny; stare pliki przepisuje się
//!   skryptem migracji, czytniki obsługują tylko bieżący major;
//! - jednostki w nazwach kolumn (`_ns`).
//!
//! Kolumny wspólne: `n` u32, `types` u8, `spec_seed` u64, `placement_seed` u64,
//! `step_offset` u32, `interactions` list<f32> (macierz sił wierszami, `types × types`),
//! `candidates_mean` f32, `rep` u16. Dalej `sim`: `candidates` f32 (kandydaci w tym kroku),
//! `step_ns` i `<system>_ns` u64; `grid`: `op` tekst, `state_revision` tekst, `ns` u64.
//!
//! Historia wersji: 1.1 dodała `rebuild_grid_ns` (siatka budowana co krok od nowa); w `grid`
//! od tej wersji operacja `rebuild` zamiast `move` (wartości `op`, nie kolumny).

use std::fs::{self, File};
use std::path::PathBuf;
use std::sync::Arc;

use arrow_array::builder::{Float32Builder, ListBuilder};
use arrow_array::{
  ArrayRef, Float32Array, RecordBatch, StringArray, UInt8Array, UInt16Array, UInt32Array, UInt64Array,
};
use arrow_schema::{DataType, Field, Schema, SchemaRef};
use parquet::arrow::ArrowWriter;
use parquet::arrow::arrow_writer::ArrowWriterOptions;
use parquet::basic::{Compression, ZstdLevel};
use parquet::file::metadata::KeyValue;
use parquet::file::properties::WriterProperties;

use super::{SYSTEM_NAMES, SYSTEMS, Sample};

pub const SCHEMA: &str = "partikl-bench";
pub const SCHEMA_VERSION: &str = "1.1";

/// Metadane przebiegu zapisywane w stopce pliku.
#[derive(Debug, Clone)]
pub struct Run {
  pub bench: String,
  pub revision: String,
  pub started: String,
  pub warmup: usize,
  pub measured: usize,
  pub world: [f32; 2],
}

impl Run {
  fn key_values(&self) -> Vec<KeyValue> {
    [
      ("schema", SCHEMA.to_string()),
      ("schema_version", SCHEMA_VERSION.to_string()),
      ("bench", self.bench.clone()),
      ("revision", self.revision.clone()),
      ("started", self.started.clone()),
      ("warmup", self.warmup.to_string()),
      ("measured", self.measured.to_string()),
      ("world", format!("[{}, {}]", self.world[0], self.world[1])),
    ]
    .into_iter()
    .map(|(k, v)| KeyValue::new(k.to_string(), v))
    .collect()
  }
}

/// Plik wyników w budowie: zapis do `<plik>.partial`, zmiana nazwy przy `finish`, więc
/// przerwany przebieg nie zostawia pliku wyglądającego na kompletny.
pub struct ResultsFile {
  writer: ArrowWriter<File>,
  partial: PathBuf,
  pub path: PathBuf,
}

impl ResultsFile {
  pub fn create(path: PathBuf, schema: SchemaRef, run: &Run) -> Self {
    let partial = path.with_extension("parquet.partial");
    let file = File::create(&partial).expect("cannot create output file");
    let props = WriterProperties::builder()
      .set_compression(Compression::ZSTD(ZstdLevel::try_new(9).unwrap()))
      .set_key_value_metadata(Some(run.key_values()))
      .build();
    // Bez kopii schematu Arrow w stopce (`ARROW:schema`, ok. 1 KB): schemat Parquet wystarcza.
    let options = ArrowWriterOptions::new().with_properties(props).with_skip_arrow_metadata(true);
    let writer =
      ArrowWriter::try_new_with_options(file, schema, options).expect("cannot start parquet writer");
    ResultsFile { writer, partial, path }
  }

  pub fn write(&mut self, batch: &RecordBatch) {
    self.writer.write(batch).expect("cannot write results");
  }

  pub fn finish(self) {
    self.writer.close().expect("cannot finish results file");
    fs::rename(&self.partial, &self.path).expect("cannot rename results file");
  }
}

/// Kolumny wspólne dla obu benchmarków, wiersz na pomiar.
#[derive(Default)]
struct SampleColumns {
  n: Vec<u32>,
  types: Vec<u8>,
  spec_seed: Vec<u64>,
  placement_seed: Vec<u64>,
  step_offset: Vec<u32>,
  interactions: Vec<Vec<f32>>,
  candidates_mean: Vec<f32>,
  rep: Vec<u16>,
}

impl SampleColumns {
  fn fields() -> Vec<Field> {
    vec![
      Field::new("n", DataType::UInt32, false),
      Field::new("types", DataType::UInt8, false),
      Field::new("spec_seed", DataType::UInt64, false),
      Field::new("placement_seed", DataType::UInt64, false),
      Field::new("step_offset", DataType::UInt32, false),
      Field::new("interactions", DataType::List(Arc::new(Field::new("item", DataType::Float32, true))), false),
      Field::new("candidates_mean", DataType::Float32, false),
      Field::new("rep", DataType::UInt16, false),
    ]
  }

  fn push(&mut self, sample: &SampleKey, rep: usize) {
    self.n.push(sample.n as u32);
    self.types.push(sample.types as u8);
    self.spec_seed.push(sample.spec_seed);
    self.placement_seed.push(sample.placement_seed);
    self.step_offset.push(sample.step_offset as u32);
    self.interactions.push(sample.interactions.clone());
    self.candidates_mean.push(sample.candidates_mean as f32);
    self.rep.push(rep as u16);
  }

  fn arrays(&mut self) -> Vec<ArrayRef> {
    let mut list = ListBuilder::new(Float32Builder::new());
    for row in self.interactions.drain(..) {
      list.values().append_slice(&row);
      list.append(true);
    }
    vec![
      Arc::new(UInt32Array::from(std::mem::take(&mut self.n))),
      Arc::new(UInt8Array::from(std::mem::take(&mut self.types))),
      Arc::new(UInt64Array::from(std::mem::take(&mut self.spec_seed))),
      Arc::new(UInt64Array::from(std::mem::take(&mut self.placement_seed))),
      Arc::new(UInt32Array::from(std::mem::take(&mut self.step_offset))),
      Arc::new(list.finish()),
      Arc::new(Float32Array::from(std::mem::take(&mut self.candidates_mean))),
      Arc::new(UInt16Array::from(std::mem::take(&mut self.rep))),
    ]
  }
}

/// Identyfikacja próbki w wynikach: stan symulacji i jego opis.
#[derive(Debug, Clone)]
pub struct SampleKey {
  pub n: usize,
  pub types: usize,
  pub spec_seed: u64,
  pub placement_seed: u64,
  pub step_offset: usize,
  /// Macierz sił wierszami.
  pub interactions: Vec<f32>,
  pub candidates_mean: f64,
}

impl SampleKey {
  pub fn new(sample: &Sample, step_offset: usize, candidates_mean: f64) -> Self {
    SampleKey {
      n: sample.n,
      types: sample.types,
      spec_seed: sample.spec_seed,
      placement_seed: sample.placement_seed,
      step_offset,
      interactions: sample.spec().interactions.iter().flat_map(|i| i.force_coeffs.iter().copied()).collect(),
      candidates_mean,
    }
  }
}

/// Wiersze benchmarku `sim`.
#[derive(Default)]
pub struct SimRows {
  sample: SampleColumns,
  candidates: Vec<f32>,
  step_ns: Vec<u64>,
  system_ns: [Vec<u64>; SYSTEMS],
}

impl SimRows {
  pub fn schema() -> SchemaRef {
    let mut fields = SampleColumns::fields();
    fields.push(Field::new("candidates", DataType::Float32, false));
    fields.push(Field::new("step_ns", DataType::UInt64, false));
    for name in SYSTEM_NAMES {
      fields.push(Field::new(format!("{name}_ns"), DataType::UInt64, false));
    }
    Arc::new(Schema::new(fields))
  }

  pub fn push(&mut self, sample: &SampleKey, rep: usize, candidates: f64, step_ns: u64, system_ns: [u64; SYSTEMS]) {
    self.sample.push(sample, rep);
    self.candidates.push(candidates as f32);
    self.step_ns.push(step_ns);
    for (column, ns) in self.system_ns.iter_mut().zip(system_ns) {
      column.push(ns);
    }
  }

  /// Wiersze jako paczka do zapisu; bufor zostaje pusty.
  pub fn take_batch(&mut self) -> RecordBatch {
    let mut arrays = self.sample.arrays();
    arrays.push(Arc::new(Float32Array::from(std::mem::take(&mut self.candidates))));
    arrays.push(Arc::new(UInt64Array::from(std::mem::take(&mut self.step_ns))));
    for column in &mut self.system_ns {
      arrays.push(Arc::new(UInt64Array::from(std::mem::take(column))));
    }
    RecordBatch::try_new(Self::schema(), arrays).unwrap()
  }
}

/// Wiersze benchmarku `grid`.
#[derive(Default)]
pub struct GridRows {
  sample: SampleColumns,
  op: Vec<String>,
  state_revision: Vec<String>,
  ns: Vec<u64>,
}

impl GridRows {
  pub fn schema() -> SchemaRef {
    let mut fields = SampleColumns::fields();
    fields.push(Field::new("op", DataType::Utf8, false));
    fields.push(Field::new("state_revision", DataType::Utf8, false));
    fields.push(Field::new("ns", DataType::UInt64, false));
    Arc::new(Schema::new(fields))
  }

  pub fn push(&mut self, sample: &SampleKey, rep: usize, op: &str, state_revision: &str, ns: u64) {
    self.sample.push(sample, rep);
    self.op.push(op.to_string());
    self.state_revision.push(state_revision.to_string());
    self.ns.push(ns);
  }

  pub fn take_batch(&mut self) -> RecordBatch {
    let mut arrays = self.sample.arrays();
    arrays.push(Arc::new(StringArray::from(std::mem::take(&mut self.op))));
    arrays.push(Arc::new(StringArray::from(std::mem::take(&mut self.state_revision))));
    arrays.push(Arc::new(UInt64Array::from(std::mem::take(&mut self.ns))));
    RecordBatch::try_new(Self::schema(), arrays).unwrap()
  }
}
