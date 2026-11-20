use std::f32::consts::PI;

use bevy::prelude::*;

use super::rotation_from_velocity;

/// Dawny wzór: kąt z `angle_to` i `Quat::from_rotation_z`.
fn reference(velocity: Vec2) -> Quat {
  Quat::from_rotation_z(-velocity.angle_to(Vec2::X))
}

/// Ten sam obrót: kwaterniony `q` i `−q` są równoważne.
fn assert_same_rotation(actual: Quat, expected: Quat, velocity: Vec2) {
  let dot = actual.dot(expected).abs();
  assert!(
    (1.0 - dot) < 1e-6,
    "v = {velocity}: {actual} vs {expected} (|dot| = {dot})"
  );
  assert!((actual.length() - 1.0).abs() < 1e-6, "v = {velocity}: |q| = {}", actual.length());
}

#[test]
fn rotation_matches_angle_formula_around_the_circle() {
  for length in [0.011, 0.5, 1.0, 37.0, 4000.0] {
    for i in 0..3600 {
      let angle = -PI + i as f32 * (2.0 * PI / 3600.0);
      let velocity = Vec2::from_angle(angle) * length;
      assert_same_rotation(rotation_from_velocity(velocity), reference(velocity), velocity);
    }
  }
}

#[test]
fn rotation_near_and_at_the_negative_x_axis() {
  for velocity in [
    Vec2::new(-1.0, 0.0),
    Vec2::new(-1.0, -0.0),
    Vec2::new(-1.0, 1e-4),
    Vec2::new(-1.0, -1e-4),
    Vec2::new(-1.0, 1e-20),
    Vec2::new(-250.0, 1e-3),
  ] {
    assert_same_rotation(rotation_from_velocity(velocity), reference(velocity), velocity);
  }
}

#[test]
fn rotation_turns_x_axis_to_velocity_direction() {
  for velocity in [Vec2::new(3.0, 4.0), Vec2::new(-2.0, 0.5), Vec2::new(0.0, -7.0)] {
    let turned = rotation_from_velocity(velocity) * Vec3::X;
    let expected = velocity.normalize().extend(0.0);
    assert!(turned.distance(expected) < 1e-6, "v = {velocity}: {turned} vs {expected}");
  }
}
