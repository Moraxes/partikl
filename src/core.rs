use bevy::prelude::*;

use std::collections::HashMap;

pub use crate::args::*;

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
  pub color: [f32; 4],
  pub mesh_handle: Handle<Mesh>
}
#[derive(Component, Default, Clone, Copy)]
pub struct CategoryId(pub usize);

#[derive(Default)]
pub struct SimRegion {
  top_right: Vec2,
  pub granularity: f32,
  pub index: HashMap<(i32, i32), Vec<Entity>>,
}

impl SimRegion {
  const OFFSETS: [i32; 3] = [-1, 0, 1];

  pub fn new(width: f32, height: f32, granularity: f32) -> SimRegion {
    SimRegion {
      top_right: Vec2::new(width/2.0, height/2.0),
      granularity,
      ..Default::default()
    }
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

  fn get_wrapped_buckets(&self, ix: i32, iy: i32) -> Vec<(i32, i32)> {
    let right = (self.top_right.x / self.granularity).round() as i32;
    let top = (self.top_right.y / self.granularity).round() as i32;
    let left = (-self.top_right.x / self.granularity).round() as i32;
    let bottom = (-self.top_right.y / self.granularity).round() as i32;
    let mut result = vec![(ix, iy); 4];
    let mut count = 1;

    if ix <= left {
      result[count].0 += right - left;
      count += 1;
    } else if ix >= right {
      result[count].0 -= right - left;
      count += 1;
    }

    if iy <= bottom {
      result[count].1 += top - bottom;
      count += 1;
    } else if iy >= top {
      result[count].1 -= top - bottom;
      count += 1;
    }

    if (ix <= left || ix >= right) && (iy <= bottom || iy >= top) {
      result[count].0 = result[1].0;
      result[count].1 = result[2].1;
      count += 1;
    }

    result.truncate(count);
    result
  }

  pub fn insert_entity(&mut self, entity: Entity, x: f32, y: f32) {
    self.index.entry(self.bucket_coords(x, y)).or_default().push(entity)
  }

  pub fn remove_entity(&mut self, entity: Entity, x_old: f32, y_old: f32) {
    let bucket_old = self.index.get_mut(&self.bucket_coords(x_old, y_old))
      .expect("no bucket");
    let idx = bucket_old.iter().position(|&e| e == entity)
      .expect("entity not in bucket");
    bucket_old.swap_remove(idx);
  }

  pub fn move_entity(&mut self, entity: Entity, x_old: f32, y_old: f32, x_new: f32, y_new: f32) {
    let old_bucket = self.bucket_coords(x_old, y_old);
    let new_bucket = self.bucket_coords(x_new, y_new);
    if old_bucket == new_bucket {
      return;
    }
    self.remove_entity(entity, x_old, y_old);
    self.insert_entity(entity, x_new, y_new);
  }

  pub fn get_bucket_with_boundary(&self, x: f32, y: f32) -> impl Iterator<Item=Entity> + '_ {
    let (ix, iy) = self.bucket_coords(x, y);
    Self::OFFSETS.iter().cloned()
      .flat_map(move |xoff| Self::OFFSETS.iter().flat_map(move |&yoff|
        self.get_wrapped_buckets(ix + xoff, iy + yoff))
      ).flat_map(|offset| self.index.get(&offset))
      .flatten()
      .cloned()
  }

  pub fn bucket_coords(&self, x: f32, y: f32) -> (i32, i32) {
    ((x / self.granularity).round() as i32, (y / self.granularity).round() as i32)
  }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum SimState {
  Running,
  Paused,
}

#[derive(Component, Default, Debug)]
pub struct Selection;
#[derive(Component, Default, Debug)]
pub struct Highlight;
#[derive(Component, Default, Debug)]
pub struct MainCamera;
