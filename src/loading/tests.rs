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

#[test]
fn current_full_spec_loads_all_rule_params() {
  let path = temp_file(
    "full-current",
    "(
      interactions: [
        (force_coeffs: [100.0, -50.0]),
        (force_coeffs: [25.0, 10.0]),
      ],
      params: (
        repulsion_radius: 1.5,
        kernel_middle: 28.0,
        kernel_width: 12.0,
        attraction_scale: 12.0,
        attraction_sum_ref: 6.0,
        friction: 0.02,
        separation_radius: 3.0,
        separation_stiffness: 15.0,
        separation_saturation: 5.0,
        vision_radius: 35.0,
        cohesion_stiffness: 0.8,
        alignment_rate: 0.9,
        target_speed: 30.0,
        drive_rate: 0.5,
        fov_cos: 0.5,
      ),
    )",
  );
  let spec = load_spec(&path).unwrap();
  assert_eq!(spec.flat_matrix(), [100.0, -50.0, 25.0, 10.0]);
  assert_eq!(spec.params.repulsion_radius, 1.5);
  assert_eq!(spec.params.kernel_middle, 28.0);
  assert_eq!(spec.params.kernel_width, 12.0);
  assert_eq!(spec.params.attraction_scale, 12.0);
  assert_eq!(spec.params.attraction_sum_ref, 6.0);
  assert_eq!(spec.params.friction, 0.02);
  assert_eq!(spec.params.separation_radius, 3.0);
  assert_eq!(spec.params.separation_stiffness, 15.0);
  assert_eq!(spec.params.separation_saturation, 5.0);
  assert_eq!(spec.params.vision_radius, 35.0);
  assert_eq!(spec.params.cohesion_stiffness, 0.8);
  assert_eq!(spec.params.alignment_rate, 0.9);
  assert_eq!(spec.params.target_speed, 30.0);
  assert_eq!(spec.params.drive_rate, 0.5);
  assert_eq!(spec.params.fov_cos, 0.5);
}

#[test]
fn legacy_unknown_fields_are_rejected_by_deny_unknown_fields() {
  for field in [
    "repulsion_strength: 100.0",
    "adaptive_neighbours: 64.0",
    "adaptive_range_max: 2.0",
    "crowding_smoothing: 0.5",
    "average_neighbours: 64.0",
    "separation_weight: 2.0",
  ] {
    let name = field.split(':').next().unwrap();
    let path = temp_file(
      &format!("legacy-unknown-{name}"),
      &format!("(interactions: [(force_coeffs: [1.0])], params: ({field}))"),
    );
    let err = load_spec(&path).unwrap_err();
    assert!(
      matches!(err, LoadError::Parse { .. }),
      "{field}: oczekiwano LoadError::Parse, dostano {err}"
    );
  }
}

#[test]
fn param_limits_are_enforced_on_load() {
  for (name, params) in [
    ("zero-width", "kernel_width: 0.0"),
    ("zero-sum-ref", "attraction_sum_ref: 0.0"),
    ("zero-saturation", "separation_saturation: 0.0"),
    ("vision-too-far", "vision_radius: 100.0"),
    ("separation-too-far", "separation_radius: 100.0"),
  ] {
    let path = temp_file(
      name,
      &format!("(interactions: [(force_coeffs: [1.0])], params: ({params}))"),
    );
    assert!(
      matches!(load_spec(&path), Err(LoadError::Malformed { .. })),
      "{name}: oczekiwano LoadError::Malformed"
    );
  }
}

/// Pusty katalog tymczasowy dla testu `name`.
fn temp_dir(name: &str) -> PathBuf {
  let dir = std::env::temp_dir().join(format!("partikl-loading-{}-{name}", std::process::id()));
  let _ = std::fs::remove_dir_all(&dir);
  std::fs::create_dir_all(&dir).unwrap();
  dir
}

#[test]
fn free_path_skips_existing_files() {
  let dir = temp_dir("free-path");
  let names: Vec<_> = (0..3)
    .map(|_| {
      let path = free_path(&dir, "spec");
      std::fs::write(&path, "").unwrap();
      path.file_name().unwrap().to_str().unwrap().to_owned()
    })
    .collect();
  assert_eq!(names, ["spec.ron", "spec-2.ron", "spec-3.ron"]);
  std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn timestamped_dumps_do_not_overwrite_each_other() {
  let dir = temp_dir("timestamped");
  let spec = |coeff: f32| {
    ParticleSpec::new(vec![Interaction {
      force_coeffs: vec![coeff],
    }])
  };
  let first = dump_timestamped_spec(&dir, &spec(1.0)).unwrap();
  let second = dump_timestamped_spec(&dir, &spec(2.0)).unwrap();
  assert_ne!(first, second);
  assert_eq!(load_spec(&first).unwrap().flat_matrix(), [1.0]);
  assert_eq!(load_spec(&second).unwrap().flat_matrix(), [2.0]);
  std::fs::remove_dir_all(&dir).unwrap();
}
