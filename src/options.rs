//! Opcje symulacji, przełączane w trakcie (klawisze w `ui`) i ustawiane flagami przy starcie.
//!
//! Opcje deklaruje moduł reguł ([`RuleSchema`], z nagłówka źródła), nie Rust: każda opcja to lista
//! wyborów, z których część włącza flagę shadera. Każda kombinacja flag to osobny wariant pipeline'u
//! sił na GPU, więc shader nie sprawdza opcji w pętli. [`SimOptions`] trzyma tylko wybory
//! użytkownika; czego nie ustawił, ma pierwszy wybór z deklaracji.

use std::collections::BTreeMap;

use bevy::prelude::*;

/// Klawisze zajęte przez aplikację (jakość rysowania, ekran, HUD); opcje reguł ich nie biorą.
const RESERVED_KEYS: [char; 4] = ['A', 'B', 'F', 'H'];
const DIRECTIVE: &str = "//@option";

/// Jeden wybór opcji: etykieta i flaga shadera włączana przez ten wybór (jeśli jest).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
  pub label: String,
  pub flag: Option<String>,
}

/// Opcja: nazwa, klawisz przełączający (litera) i wybory; pierwszy wybór to domyślny.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionDecl {
  pub name: String,
  pub key: Option<char>,
  pub choices: Vec<Choice>,
}

impl OptionDecl {
  pub fn keycode(&self) -> Option<KeyCode> {
    Some(match self.key? {
      'A' => KeyCode::KeyA, 'B' => KeyCode::KeyB, 'C' => KeyCode::KeyC, 'D' => KeyCode::KeyD,
      'E' => KeyCode::KeyE, 'F' => KeyCode::KeyF, 'G' => KeyCode::KeyG, 'H' => KeyCode::KeyH,
      'I' => KeyCode::KeyI, 'J' => KeyCode::KeyJ, 'K' => KeyCode::KeyK, 'L' => KeyCode::KeyL,
      'M' => KeyCode::KeyM, 'N' => KeyCode::KeyN, 'O' => KeyCode::KeyO, 'P' => KeyCode::KeyP,
      'Q' => KeyCode::KeyQ, 'R' => KeyCode::KeyR, 'S' => KeyCode::KeyS, 'T' => KeyCode::KeyT,
      'U' => KeyCode::KeyU, 'V' => KeyCode::KeyV, 'W' => KeyCode::KeyW, 'X' => KeyCode::KeyX,
      'Y' => KeyCode::KeyY, 'Z' => KeyCode::KeyZ,
      _ => return None,
    })
  }

  /// Nazwa do wyświetlenia (`average_forces` → `average forces`).
  pub fn title(&self) -> String {
    self.name.replace('_', " ")
  }
}

/// Opcje zadeklarowane przez moduł reguł; zasób odświeżany przy zmianie źródła reguł.
#[derive(Resource, Debug, Clone, Default, PartialEq, Eq)]
pub struct RuleSchema {
  pub options: Vec<OptionDecl>,
}

fn is_identifier(text: &str) -> bool {
  !text.is_empty() && text.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') && !text.starts_with(|c: char| c.is_ascii_digit())
}

impl RuleSchema {
  /// Czyta deklaracje z nagłówka źródła reguł: linie `//@option nazwa [key=K]: wybór[=FLAGA], ...`.
  pub fn parse(source: &str) -> Result<RuleSchema, String> {
    let mut options: Vec<OptionDecl> = Vec::new();
    for line in source.lines().map(str::trim).filter_map(|line| line.strip_prefix(DIRECTIVE)) {
      let option = Self::parse_option(line).map_err(|reason| format!("{DIRECTIVE}{line}: {reason}"))?;
      if options.iter().any(|other| other.name == option.name) {
        return Err(format!("opcja `{}` zadeklarowana dwa razy", option.name));
      }
      if let Some(key) = option.key
        && let Some(other) = options.iter().find(|other| other.key == Some(key))
      {
        return Err(format!("klawisz {key} jest już opcji `{}`", other.name));
      }
      for flag in option.choices.iter().filter_map(|c| c.flag.as_ref()) {
        if options.iter().flat_map(|o| &o.choices).chain(&option.choices).filter(|c| c.flag.as_ref() == Some(flag)).count() > 1 {
          return Err(format!("flaga {flag} użyta dwa razy"));
        }
      }
      options.push(option);
    }
    Ok(RuleSchema { options })
  }

  fn parse_option(line: &str) -> Result<OptionDecl, String> {
    let (head, choices) = line.split_once(':').ok_or("brak `:` przed wyborami")?;
    let mut words = head.split_whitespace();
    let name = words.next().filter(|name| is_identifier(name)).ok_or("brak nazwy opcji")?.to_string();
    let mut key = None;
    for word in words {
      let letter = word.strip_prefix("key=").ok_or_else(|| format!("nieznane pole `{word}`"))?;
      let mut chars = letter.chars();
      match (chars.next(), chars.next()) {
        (Some(c), None) if c.is_ascii_uppercase() && !RESERVED_KEYS.contains(&c) => key = Some(c),
        _ => return Err(format!("klawisz `{letter}`: wielka litera poza {RESERVED_KEYS:?}")),
      }
    }
    let choices = choices
      .split(',')
      .map(|choice| {
        let (label, flag) = match choice.split_once('=') {
          Some((label, flag)) => (label.trim(), Some(flag.trim())),
          None => (choice.trim(), None),
        };
        if !is_identifier(label) {
          return Err(format!("zła etykieta wyboru `{label}`"));
        }
        if flag.is_some_and(|flag| !is_identifier(flag)) {
          return Err(format!("zła flaga `{}`", flag.unwrap()));
        }
        Ok(Choice { label: label.to_string(), flag: flag.map(str::to_string) })
      })
      .collect::<Result<Vec<_>, _>>()?;
    if choices.len() < 2 {
      return Err("opcja potrzebuje co najmniej dwóch wyborów".into());
    }
    Ok(OptionDecl { name, key, choices })
  }

  pub fn option(&self, name: &str) -> Option<&OptionDecl> {
    self.options.iter().find(|option| option.name == name)
  }

  /// Flagi shadera dla opcji: każda zadeklarowana flaga z wartością (włączona wyborem albo nie).
  pub fn flags(&self, options: &SimOptions) -> Vec<(String, bool)> {
    self
      .options
      .iter()
      .flat_map(|decl| {
        let chosen = options.choice(decl);
        decl.choices.iter().filter_map(move |choice| choice.flag.clone().map(|flag| (flag, choice == chosen)))
      })
      .collect()
  }

  /// Wszystkie kombinacje wyborów (do benchmarków wariantów).
  pub fn combinations(&self) -> Vec<SimOptions> {
    self.options.iter().fold(vec![SimOptions::default()], |partial, decl| {
      partial
        .iter()
        .flat_map(|options| decl.choices.iter().map(move |choice| options.clone().with(&decl.name, &choice.label)))
        .collect()
    })
  }
}

/// Wybory użytkownika: nazwa opcji → etykieta wyboru. Opcja bez wpisu (albo z wyborem, którego
/// reguły nie deklarują) ma pierwszy wybór z deklaracji.
#[derive(Resource, Debug, Default, Clone, PartialEq, Eq, Hash)]
pub struct SimOptions(BTreeMap<String, String>);

impl SimOptions {
  /// Ten sam zestaw z ustawioną opcją (bez sprawdzania; patrz [`SimOptions::from_overrides`]).
  pub fn with(mut self, name: &str, choice: &str) -> SimOptions {
    self.0.insert(name.into(), choice.into());
    self
  }

  /// Wybory z flag `--option nazwa=wybór`; błąd dla nieznanej opcji albo wyboru.
  pub fn from_overrides(schema: &RuleSchema, overrides: &[String]) -> Result<SimOptions, String> {
    let mut options = SimOptions::default();
    for text in overrides {
      let (name, choice) = text.split_once('=').ok_or_else(|| format!("`{text}`: oczekiwano nazwa=wybór"))?;
      let decl = schema.option(name).ok_or_else(|| {
        let known: Vec<_> = schema.options.iter().map(|o| o.name.as_str()).collect();
        format!("reguły nie mają opcji `{name}` (mają: {})", known.join(", "))
      })?;
      if !decl.choices.iter().any(|c| c.label == choice) {
        let known: Vec<_> = decl.choices.iter().map(|c| c.label.as_str()).collect();
        return Err(format!("opcja `{name}` nie ma wyboru `{choice}` (ma: {})", known.join(", ")));
      }
      options = options.with(name, choice);
    }
    Ok(options)
  }

  /// Etykieta wybranego wyboru opcji `name` (`None`, gdy użytkownik jej nie ustawił).
  pub fn value(&self, name: &str) -> Option<&str> {
    self.0.get(name).map(String::as_str)
  }

  /// Aktualny wybór opcji.
  pub fn choice<'a>(&self, decl: &'a OptionDecl) -> &'a Choice {
    self.value(&decl.name).and_then(|label| decl.choices.iter().find(|c| c.label == label)).unwrap_or(&decl.choices[0])
  }

  /// Następny wybór opcji (cyklicznie), dla klawisza przełączającego.
  pub fn cycle(&mut self, decl: &OptionDecl) {
    let current = decl.choices.iter().position(|c| c == self.choice(decl)).unwrap_or(0);
    let next = &decl.choices[(current + 1) % decl.choices.len()];
    self.0.insert(decl.name.clone(), next.label.clone());
  }
}

/// Symulacja biegnie albo stoi (spacja w `ui`).
#[derive(States, Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum SimState {
  #[default]
  Running,
  Paused,
}

#[cfg(test)]
mod tests;
