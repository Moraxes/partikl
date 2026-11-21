//! Moduł reguł fizyki w shaderze sił. `step_driver.wesl` importuje zawsze ten sam moduł
//! (`rules/model.wesl`), a jego źródło wybiera Rust ([`RuleSource`]): wbudowane reguły domyślne,
//! reguły diagnostyczne (testy silnika siatki) albo plik z dysku (`--rules path.wesl`), który jest
//! obserwowany i przeładowywany po zapisie. W M3 źródłem będzie kod wygenerowany z modelu.
//!
//! Źródło ma kontrakt z nagłówka `default_rules.wesl` i importuje moduły względem
//! `shaders/rules/` (np. `super::matrix`, `super::super::engine::bindings`).
//!
//! Wymiana źródła podmienia zasób `Shader`, więc pipeline'y sił kompilują się od nowa; błąd
//! kompilacji (np. literówka w pliku) trafia do logu i kroki czekają na poprawkę.

use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use bevy::prelude::*;
use bevy::shader::Shader;

use crate::loading::LoadError;

/// Ścieżka shadera, pod którą żyje moduł reguł; jej moduł WESL to `partikl::gpu::shaders::rules::model`.
const MODULE_PATH: &str = "embedded://partikl/gpu/shaders/rules/model.wesl";
const BUILTIN: &str = include_str!("shaders/rules/default_rules.wesl");
const DIAGNOSTIC: &str = include_str!("shaders/rules/diagnostic_rules.wesl");
/// Jak często sprawdzamy, czy plik reguł się zmienił.
const POLL: Duration = Duration::from_millis(500);

/// Skąd pochodzi kod modułu reguł.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub enum RuleSource {
  /// Wbudowane reguły fizyki (`rules/default_rules.wesl`).
  #[default]
  Builtin,
  /// Nic nie poruszają, zapisują w cząstce wynik sąsiedztwa (`rules/diagnostic_rules.wesl`); do
  /// testów silnika siatki bez fizyki.
  Diagnostic,
  /// Plik `.wesl` z dysku, obserwowany.
  File(PathBuf),
}

impl RuleSource {
  /// Kod modułu; dla pliku czyta go z dysku.
  pub fn read(&self) -> Result<Cow<'static, str>, LoadError> {
    match self {
      RuleSource::Builtin => Ok(BUILTIN.into()),
      RuleSource::Diagnostic => Ok(DIAGNOSTIC.into()),
      RuleSource::File(path) => {
        std::fs::read_to_string(path).map(Cow::Owned).map_err(|source| LoadError::Io { path: path.clone(), source })
      }
    }
  }
}

/// Zasób `Shader` z modułem reguł i to, co z niego jest zastosowane.
#[derive(Resource)]
struct RulesModule {
  handle: Handle<Shader>,
  applied: RuleSource,
  /// Czas modyfikacji obserwowanego pliku w chwili ostatniego wczytania.
  modified: Option<SystemTime>,
  poll: Timer,
}

fn modified(path: &Path) -> Option<SystemTime> {
  std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

pub struct RulesPlugin;

impl Plugin for RulesPlugin {
  fn build(&self, app: &mut App) {
    app.init_resource::<RuleSource>();
    // Moduł musi być w zasobach shaderów, zanim ruszy kompilacja `step_driver`, więc powstaje
    // od razu, a nie w systemie.
    let source = app.world().resource::<RuleSource>().clone();
    let code = source.read().unwrap_or_else(|error| panic!("reguły: {error}"));
    let handle = app.world_mut().resource_mut::<Assets<Shader>>().add(Shader::from_wesl(code, MODULE_PATH));
    let modified = if let RuleSource::File(path) = &source { modified(path) } else { None };
    app.insert_resource(RulesModule { handle, applied: source, modified, poll: Timer::new(POLL, TimerMode::Repeating) });
    app.add_systems(Update, reload_rules);
  }
}

/// Podmienia moduł reguł, gdy zmieniło się źródło albo plik reguł na dysku.
fn reload_rules(
  source: Res<RuleSource>,
  time: Res<Time<Real>>,
  mut module: ResMut<RulesModule>,
  mut shaders: ResMut<Assets<Shader>>,
) {
  let changed_source = *source != module.applied;
  let mut changed_file = false;
  if let RuleSource::File(path) = &*source {
    if module.poll.tick(time.delta()).just_finished() {
      let now = modified(path);
      changed_file = now.is_some() && now != module.modified;
    }
  }
  if !changed_source && !changed_file {
    return;
  }
  module.applied = source.clone();
  module.modified = if let RuleSource::File(path) = &*source { modified(path) } else { None };
  match source.read() {
    Ok(code) => {
      info!("reguły: wczytano {source:?}");
      let _ = shaders.insert(&module.handle, Shader::from_wesl(code, MODULE_PATH));
    }
    Err(error) => error!("reguły: {error}; zostaje poprzedni moduł"),
  }
}
