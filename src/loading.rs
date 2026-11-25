use chrono::offset::Utc;

use rand::{Rng, RngExt};

use ron::options::Options;
use ron::ser::PrettyConfig;

use serde::Deserialize;

use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::fs::File;
use std::path::{Path, PathBuf};

use crate::args;
use crate::spec::{Interaction, ParticleSpec};

/// Błąd wczytania albo zapisu specyfikacji; komunikat zawiera ścieżkę pliku.
#[derive(Debug)]
pub enum LoadError {
  Io {
    path: PathBuf,
    source: std::io::Error,
  },
  Parse {
    path: PathBuf,
    /// W pudełku: `SpannedError` ma ponad 100 B, a `LoadError` wraca w każdym `Result`.
    source: Box<ron::error::SpannedError>,
  },
  Write {
    path: PathBuf,
    source: ron::Error,
  },
  Malformed {
    path: PathBuf,
    reason: String,
  },
  /// Błędne argumenty specyfikacji (kilka plików, nadpisanie `PATH=VALUE` nie pasuje do pól).
  Args(String),
}

impl Display for LoadError {
  fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
    match self {
      LoadError::Io { path, source } => write!(f, "{}: {source}", path.display()),
      LoadError::Parse { path, source } => write!(
        f,
        "{}: nie da się wczytać specyfikacji: {source}",
        path.display()
      ),
      LoadError::Write { path, source } => write!(
        f,
        "{}: nie da się zapisać specyfikacji: {source}",
        path.display()
      ),
      LoadError::Malformed { path, reason } => {
        write!(f, "{}: błędna specyfikacja: {reason}", path.display())
      }
      LoadError::Args(reason) => write!(f, "{reason}"),
    }
  }
}

impl Error for LoadError {
  fn source(&self) -> Option<&(dyn Error + 'static)> {
    match self {
      LoadError::Io { source, .. } => Some(source),
      LoadError::Parse { source, .. } => Some(source.as_ref()),
      LoadError::Write { source, .. } => Some(source),
      LoadError::Malformed { .. } | LoadError::Args(_) => None,
    }
  }
}

/// Zawartość pliku `.ron`: pełna specyfikacja `(interactions: [...], params: (...))` albo, jak w
/// starszych plikach, sama lista wierszy macierzy sił.
#[derive(Deserialize)]
#[serde(untagged)]
enum SpecFile {
  Full(ParticleSpec),
  Matrix(Vec<Interaction>),
}

pub fn get_particle_spec(program_args: &args::ProgramArgs) -> Result<ParticleSpec, LoadError> {
  let files = program_args.spec_files();
  if files.len() > 1 {
    return Err(LoadError::Args(format!(
      "podano {} pliki specyfikacji, można najwyżej jeden",
      files.len()
    )));
  }
  let overrides = program_args.spec_overrides();
  if let Some(path) = files.first() {
    apply_overrides(load_spec(path)?, &overrides)
  } else {
    use rand::SeedableRng;

    let mut rng = if let Some(seed) = program_args.interaction_seed {
      // Ziarno u64 powtórzone na całą długość seeda (zachowuje dotychczasowe wyniki).
      let mut full_seed = [0u8; 32];
      full_seed
        .as_chunks_mut::<8>()
        .0
        .iter_mut()
        .for_each(|chunk| chunk.copy_from_slice(&seed.to_le_bytes()));
      rand::rngs::SmallRng::from_seed(full_seed)
    } else {
      rand::rngs::SmallRng::from_rng(&mut rand::rng())
    };
    let type_count = program_args.num_types;
    // Zapis po nadpisaniach: plik odtwarza dokładnie tę symulację.
    let particle_spec = apply_overrides(generate_particle_spec(&mut rng, type_count), &overrides)?;
    if !program_args.no_dump_interaction_spec {
      dump_timestamped_spec(Path::new(""), &particle_spec)?;
    }
    Ok(particle_spec)
  }
}

/// Nadpisuje pola specyfikacji (`PATH=VALUE`, np. `params.repulsion_radius=1`,
/// `interactions.0.force_coeffs.2=-300`): ścieżka idzie po polach jak w pliku `.ron`, pozycje
/// list to liczby, a wartość jest w składni `.ron`. Wynik przechodzi tę samą kontrolę co plik.
pub fn apply_overrides(
  spec: ParticleSpec,
  overrides: &[(&str, &str)],
) -> Result<ParticleSpec, LoadError> {
  if overrides.is_empty() {
    return Ok(spec);
  }
  let error =
    |path: &str, value: &str, reason: String| LoadError::Args(format!("{path}={value}: {reason}"));
  let text = ron::to_string(&spec).map_err(|e| LoadError::Args(e.to_string()))?;
  let mut tree: ron::Value = ron::from_str(&text).map_err(|e| LoadError::Args(e.to_string()))?;
  for &(path, value) in overrides {
    let parsed: ron::Value = ron::from_str(value)
      .map_err(|e| error(path, value, format!("wartość nie jest poprawnym RON: {e}")))?;
    let mut node = &mut tree;
    for segment in path.split('.') {
      node = match node {
        ron::Value::Map(map) => {
          let names: Vec<String> = map
            .keys()
            .filter_map(|k| match k {
              ron::Value::String(s) => Some(s.clone()),
              _ => None,
            })
            .collect();
          map
            .get_mut(&ron::Value::String(segment.into()))
            .ok_or_else(|| {
              error(
                path,
                value,
                format!("nie ma pola `{segment}` (są: {})", names.join(", ")),
              )
            })?
        }
        ron::Value::Seq(items) => {
          let len = items.len();
          segment
            .parse::<usize>()
            .ok()
            .and_then(|i| items.get_mut(i))
            .ok_or_else(|| {
              error(
                path,
                value,
                format!("`{segment}`: oczekiwano pozycji 0..{len}"),
              )
            })?
        }
        _ => {
          return Err(error(
            path,
            value,
            format!("`{segment}`: wartość wyżej nie ma pól"),
          ));
        }
      };
    }
    *node = parsed;
    // Sprawdzenie typu od razu, żeby błąd wskazał nadpisanie, które go spowodowało.
    tree
      .clone()
      .into_rust::<ParticleSpec>()
      .map_err(|e| error(path, value, e.to_string()))?;
  }
  let spec: ParticleSpec = tree
    .into_rust()
    .map_err(|e| LoadError::Args(format!("nadpisania specyfikacji: {e}")))?;
  validate_spec(&spec)
    .map_err(|reason| LoadError::Args(format!("specyfikacja po nadpisaniach: {reason}")))?;
  Ok(spec)
}

/// Wczytuje i sprawdza specyfikację z pliku `.ron`.
pub fn load_spec(path: &Path) -> Result<ParticleSpec, LoadError> {
  let file = File::open(path).map_err(|source| LoadError::Io {
    path: path.into(),
    source,
  })?;
  let parsed: SpecFile =
    Options::default()
      .from_reader(file)
      .map_err(|source| LoadError::Parse {
        path: path.into(),
        source: Box::new(source),
      })?;
  let spec = match parsed {
    SpecFile::Full(spec) => spec,
    SpecFile::Matrix(interactions) => ParticleSpec::new(interactions),
  };
  validate_spec(&spec).map_err(|reason| LoadError::Malformed {
    path: path.into(),
    reason,
  })?;
  Ok(spec)
}

/// Zapisuje specyfikację (macierz i parametry reguł) do pliku `.ron`.
pub fn dump_spec(path: &Path, spec: &ParticleSpec) -> Result<(), LoadError> {
  let file = File::create(path).map_err(|source| LoadError::Io {
    path: path.into(),
    source,
  })?;
  Options::default()
    .to_io_writer_pretty(file, spec, PrettyConfig::default())
    .map_err(|source| LoadError::Write {
      path: path.into(),
      source,
    })
}

/// Zapisuje `spec` w `dir` jako `spec-<czas UTC>.ron`, bez nadpisywania istniejących plików, i
/// zwraca ścieżkę.
pub fn dump_timestamped_spec(dir: &Path, spec: &ParticleSpec) -> Result<PathBuf, LoadError> {
  let stem = format!("spec-{}", Utc::now().format("%F-%H-%M-%S"));
  let path = free_path(dir, &stem);
  dump_spec(&path, spec)?;
  Ok(path)
}

/// `<stem>.ron` w `dir`, a gdy zajęte: `<stem>-2.ron`, `<stem>-3.ron`, …
fn free_path(dir: &Path, stem: &str) -> PathBuf {
  std::iter::once(format!("{stem}.ron"))
    .chain((2..).map(|n| format!("{stem}-{n}.ron")))
    .map(|name| dir.join(name))
    .find(|path| !path.exists())
    .expect("nieskończony ciąg nazw")
}

fn validate_spec(spec: &ParticleSpec) -> Result<(), String> {
  if spec.interactions.is_empty() {
    return Err("brak typów cząstek".into());
  }
  validate_interaction_list(&spec.interactions).map_err(|e| e.to_string())?;
  spec.params.validate()
}

fn validate_interaction_list(
  interactions: &[Interaction],
) -> Result<(), MalformedInteractionError> {
  let total_interactions = interactions.len();
  interactions
    .iter()
    .try_for_each(|interaction| validate_single_interaction(interaction, total_interactions))
}

fn validate_single_interaction(
  interaction: &Interaction,
  total_interactions: usize,
) -> Result<(), MalformedInteractionError> {
  let total_coeffs = interaction.force_coeffs.len();
  if total_coeffs == total_interactions {
    Ok(())
  } else {
    Err(MalformedInteractionError {
      total_interactions,
      total_coeffs,
    })
  }
}

pub(crate) fn generate_particle_spec(rng: &mut impl Rng, type_count: usize) -> ParticleSpec {
  let interactions = (0..type_count)
    .map(|_| generate_single_interaction(rng, type_count))
    .collect();
  ParticleSpec::new(interactions)
}

fn generate_single_interaction(rng: &mut impl Rng, type_count: usize) -> Interaction {
  Interaction {
    force_coeffs: (0..type_count)
      .map(|_| 1000.0 * rng.random::<f32>() - 500.0)
      .collect(),
  }
}

#[derive(Debug)]
struct MalformedInteractionError {
  total_interactions: usize,
  total_coeffs: usize,
}

impl Display for MalformedInteractionError {
  fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
    f.write_fmt(format_args!(
      "expected {} coefficients, got {}",
      self.total_interactions, self.total_coeffs
    ))
  }
}

impl Error for MalformedInteractionError {}

#[cfg(test)]
mod tests;
