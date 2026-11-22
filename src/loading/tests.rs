use super::*;

fn temp_file(name: &str, contents: &str) -> PathBuf {
  let path =
    std::env::temp_dir().join(format!("partikl-loading-{}-{name}.ron", std::process::id()));
  std::fs::write(&path, contents).unwrap();
  path
}

#[test]
fn legacy_matrix_list_gets_default_params() {
  let path = temp_file(
    "legacy",
    "[(force_coeffs: [1.0, 2.0]), (force_coeffs: [3.0, 4.0])]",
  );
  let spec = load_spec(&path).unwrap();
  assert_eq!(spec.flat_matrix(), [1.0, 2.0, 3.0, 4.0]);
  assert_eq!(spec.params, crate::gpu::RuleParams::default());
}

#[test]
fn partial_params_keep_defaults() {
  let path = temp_file(
    "partial",
    "(interactions: [(force_coeffs: [5.0])], params: (friction: 0.5))",
  );
  let spec = load_spec(&path).unwrap();
  assert_eq!(spec.params.friction, 0.5);
  assert_eq!(
    spec.params.repulsion_radius,
    crate::gpu::RuleParams::default().repulsion_radius
  );
}

#[test]
fn dump_round_trips() {
  let mut spec = ParticleSpec::new(vec![
    Interaction {
      force_coeffs: vec![1.5, -2.0],
    },
    Interaction {
      force_coeffs: vec![0.0, 4.0],
    },
  ]);
  spec.params.friction = 0.25;
  let path = temp_file("dump", "");
  dump_spec(&path, &spec).unwrap();
  let loaded = load_spec(&path).unwrap();
  assert_eq!(loaded.flat_matrix(), spec.flat_matrix());
  assert_eq!(loaded.params, spec.params);
}

#[test]
fn errors_name_the_file_and_do_not_panic() {
  let missing = std::env::temp_dir().join("partikl-loading-does-not-exist.ron");
  assert!(matches!(load_spec(&missing), Err(LoadError::Io { .. })));

  let garbage = temp_file("garbage", "to nie jest ron");
  let error = load_spec(&garbage).unwrap_err();
  assert!(matches!(error, LoadError::Parse { .. }));
  assert!(error.to_string().contains("partikl-loading-"));

  let ragged = temp_file(
    "ragged",
    "[(force_coeffs: [1.0, 2.0]), (force_coeffs: [3.0])]",
  );
  assert!(matches!(
    load_spec(&ragged),
    Err(LoadError::Malformed { .. })
  ));

  let empty = temp_file("empty", "[]");
  assert!(matches!(
    load_spec(&empty),
    Err(LoadError::Malformed { .. })
  ));

  let unknown = temp_file(
    "unknown",
    "(interactions: [(force_coeffs: [1.0])], params: (frikcja: 1.0))",
  );
  assert!(matches!(load_spec(&unknown), Err(LoadError::Parse { .. })));

  let too_wide = temp_file(
    "wide",
    "(interactions: [(force_coeffs: [1.0])], params: (kernel_width: 100.0))",
  );
  assert!(matches!(
    load_spec(&too_wide),
    Err(LoadError::Malformed { .. })
  ));
}

#[test]
fn overrides_change_params_and_matrix_entries() {
  let spec = ParticleSpec::new(vec![
    Interaction {
      force_coeffs: vec![1.0, 2.0],
    },
    Interaction {
      force_coeffs: vec![3.0, 4.0],
    },
  ]);
  let spec = apply_overrides(
    spec,
    &[
      ("params.repulsion_radius", "1"),
      ("interactions.1.force_coeffs.0", "-300.5"),
    ],
  )
  .unwrap();
  assert_eq!(spec.params.repulsion_radius, 1.0);
  assert_eq!(spec.flat_matrix(), [1.0, 2.0, -300.5, 4.0]);
  // Pozostałe parametry bez zmian.
  let defaults = crate::gpu::RuleParams::default();
  assert_eq!(spec.params.kernel_middle, defaults.kernel_middle);
  assert_eq!(spec.params.dt, defaults.dt);
}

#[test]
fn overrides_reject_unknown_fields_and_bad_values() {
  let spec = || {
    ParticleSpec::new(vec![Interaction {
      force_coeffs: vec![1.0],
    }])
  };
  for (path, value) in [
    ("params.nope", "1"),
    ("interactions.5.force_coeffs.0", "1"),
    ("params.repulsion_radius", "abc"),
    ("params.repulsion_radius.x", "1"),
  ] {
    let error = apply_overrides(spec(), &[(path, value)]).unwrap_err();
    assert!(
      matches!(error, LoadError::Args(_)),
      "{path}={value}: {error}"
    );
  }
}
