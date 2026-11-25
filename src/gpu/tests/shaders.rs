//! Każdy plik shadera kompilowany osobno (WESL → WGSL → walidacja naga), dla kilku zestawów flag.
//! Błąd wskazuje plik i linię, w odróżnieniu od błędu pipeline'u w aplikacji, który ma tylko
//! przesunięcie w sklejonym źródle. Moduł `rules::model` (w aplikacji podaje go Rust) to tu kolejno
//! reguły wbudowane i diagnostyczne. Bez `render.wesl`: importuje moduły Bevy (`bevy_sprite_render`), więc
//! sprawdza go tylko test rysowania.

use std::borrow::Cow;
use std::path::{Path, PathBuf};

use wesl::resolver::{FileResolver, Router, VirtualResolver};
use wesl::syntax::{ModulePath, PathOrigin};
use wesl::{CompileOptions, Feature, Features};

const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/gpu/shaders");

/// Zestawy flag: oba progi liczby typów (`matrix.wesl`).
const FLAG_SETS: &[&[&str]] = &[&[], &["TYPES_AT_MOST_4"], &["TYPES_AT_MOST_8"]];

const MODELS: &[&str] = &["default_rules", "diagnostic_rules"];

fn shader_files() -> Vec<PathBuf> {
  let mut files = Vec::new();
  for dir in ["engine", "rules"] {
    for entry in std::fs::read_dir(Path::new(ROOT).join(dir)).unwrap() {
      let path = entry.unwrap().path();
      let name = path.file_name().unwrap().to_str().unwrap();
      // `model.wesl` to lokalny symlink dla edytora (moduł podaje Rust), `render.wesl` patrz wyżej.
      if path.extension().is_some_and(|e| e == "wesl")
        && name != "model.wesl"
        && name != "render.wesl"
      {
        files.push(path);
      }
    }
  }
  files.sort();
  files
}

fn compile(file: &Path, model: &str, flags: &[&str]) -> Result<(), String> {
  let mut router = Router::new();
  let mut virtual_model = VirtualResolver::new();
  let source =
    std::fs::read_to_string(Path::new(ROOT).join("rules").join(format!("{model}.wesl"))).unwrap();
  virtual_model.add_module(
    ModulePath::new(PathOrigin::Absolute, vec![]),
    Cow::Owned(source),
  );
  router.mount_resolver(
    ModulePath::new(PathOrigin::Absolute, vec!["rules".into(), "model".into()]),
    virtual_model,
  );
  router.mount_fallback_resolver(FileResolver::new(ROOT));

  let relative = file.strip_prefix(ROOT).unwrap().with_extension("");
  let main = ModulePath::new(
    PathOrigin::Absolute,
    relative
      .iter()
      .map(|c| c.to_str().unwrap().to_string())
      .collect(),
  );
  let options = CompileOptions {
    // Bez usuwania nieużywanych deklaracji: moduły bez entry pointów (biblioteki węzłów, typy)
    // też mają być sprawdzone w całości.
    strip: false,
    features: Features {
      default: Feature::Disable,
      flags: flags
        .iter()
        .map(|f| (f.to_string(), Feature::Enable))
        .collect(),
    },
    ..Default::default()
  };
  let wgsl = wesl::compile(&main, &options, &router)
    .map_err(|e| e.to_string())?
    .to_string();
  let module = naga::front::wgsl::parse_str(&wgsl).map_err(|e| e.emit_to_string(&wgsl))?;
  naga::valid::Validator::new(
    naga::valid::ValidationFlags::all(),
    naga::valid::Capabilities::all(),
  )
  .validate(&module)
  .map_err(|e| e.emit_to_string(&wgsl))?;
  Ok(())
}

#[test]
fn every_shader_file_compiles_with_every_flag_set() {
  let files = shader_files();
  assert!(
    files.len() >= 10,
    "znaleziono tylko {} plików shaderów",
    files.len()
  );
  // Ten sam błąd (np. w module importowanym przez wiele plików) raz, z listą miejsc, gdzie wystąpił.
  let mut failures: std::collections::BTreeMap<String, Vec<String>> = Default::default();
  for file in &files {
    for model in MODELS {
      for flags in FLAG_SETS {
        if let Err(error) = compile(file, model, flags) {
          let context = format!(
            "{} (model {model}, flagi {flags:?})",
            file.strip_prefix(ROOT).unwrap().display()
          );
          failures.entry(error).or_default().push(context);
        }
      }
    }
  }
  let report: Vec<String> = failures
    .iter()
    .map(|(error, contexts)| {
      format!(
        "{error}
przy kompilacji: {}",
        contexts.join(", ")
      )
    })
    .collect();
  assert!(
    failures.is_empty(),
    "shadery z błędami:\n\n{}",
    report.join("\n\n")
  );
}
