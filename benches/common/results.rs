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
//! Kolumny benchu `gpu`: `n` u32, `scene` tekst (od 1.2), `variant` tekst, `spec_seed` u64,
//! `step_offset` u32, `candidates_mean` f32 (od 1.2), `rep` u16, `step_ns` u64, od 1.3
//! `gpu_step_ns` i `gpu_<przebieg>_ns` u64 z pustymi polami, gdy czasu GPU nie ma (schemat w
//! `benches/gpu/main.rs`). Bench `render`: `n`, `scene`, `variant`, `spec_seed`, `step_offset`,
//! `candidates_mean`, `rep`, `frame_ns` u64 (od 1.4), `sim_speed` f32 z pustymi polami poza
//! trybem `loop` (od 1.5; `benches/render/main.rs`). Starsze pliki benchów CPU
//! (`sim`, `grid`; usunięte razem z silnikiem CPU) mają inne kolumny, opisane w historii gita
//! tego pliku; nadal czyta je `tools/bench-viz`.

use std::fs::{self, File};
use std::path::PathBuf;

use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use parquet::arrow::ArrowWriter;
use parquet::arrow::arrow_writer::ArrowWriterOptions;
use parquet::basic::{Compression, ZstdLevel};
use parquet::file::metadata::KeyValue;
use parquet::file::properties::WriterProperties;


pub const SCHEMA: &str = "partikl-bench";
pub const SCHEMA_VERSION: &str = "1.5";

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
