use bevy::prelude::*;

pub const DELTA_TIME: f64 = 0.01;
pub const VELOCITY_THRESHOLD: f32 = 0.0001;

#[derive(Component, Default, Debug)]
pub struct Acceleration(pub Vec3);
#[derive(Component, Default, Debug)]
pub struct LastPosition(pub Vec3);

#[derive(Bundle, Default)]
pub struct ParticleBundle {
  #[bundle]
  pub mesh: MeshBundle,
  pub last_pos: LastPosition,
  pub acceleration: Acceleration,
  pub category: CategoryId
}

#[derive(Default)]
pub struct Categories(pub Vec<Category>);
#[derive(Default)]
pub struct Category {
  pub force_coeffs: Vec<f32>,
  pub color: [f32; 3],
  pub mesh_handle: Handle<Mesh>
}
#[derive(Component, Default, Clone, Copy)]
pub struct CategoryId(pub usize);

#[derive(Copy, Clone)]
pub struct SimRegion {
  top_right: Vec2
}

impl SimRegion {
  pub fn new(width: f32, height: f32) -> SimRegion {
    SimRegion { top_right: Vec2::new(width/2.0, height/2.0) }
  }

  pub fn get_corrected_position_delta(&self, origin: Vec3, target: Vec3) -> Vec3 {
    let delta = target - origin;
    delta + self.get_wrap_around_adjustment(delta)
  }

  pub fn get_wrap_around_adjustment(&self, point: Vec3) -> Vec3 {
    let mut adjustment = Vec3::ZERO;
    if point.x > self.top_right.x {
      adjustment.x = -self.top_right.x;
    } else if point.x < -self.top_right.x {
      adjustment.x = self.top_right.x;
    }
    if point.y > self.top_right.y {
      adjustment.y = -self.top_right.y;
    } else if point.y < -self.top_right.y {
      adjustment.y = self.top_right.y;
    }
    2.0 * adjustment
  }
}