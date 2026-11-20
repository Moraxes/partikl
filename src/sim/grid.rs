use bevy::prelude::*;

use std::collections::HashMap;

/// Siatka przestrzenna z zawijaniem (torus) do szukania sąsiadów cząstek.
/// Świat obejmuje `[-half_size, half_size]` w obu osiach.
#[derive(Default, Resource)]
pub struct SpatialGrid {
  half_size: Vec2,
  pub granularity: f32,
  pub index: HashMap<(i32, i32), Vec<Entity>>,
}

impl SpatialGrid {
  const OFFSETS: [i32; 3] = [-1, 0, 1];

  pub fn new(width: f32, height: f32, granularity: f32) -> SpatialGrid {
    SpatialGrid {
      half_size: Vec2::new(width / 2.0, height / 2.0),
      granularity,
      ..Default::default()
    }
  }

  pub fn get_corrected_position_delta(&self, origin: Vec2, target: Vec2) -> Vec2 {
    let delta = target - origin;
    delta + self.get_wrap_around_adjustment(delta)
  }

  pub fn get_wrap_around_adjustment(&self, point: Vec2) -> Vec2 {
    let mut adjustment = Vec2::ZERO;
    if point.x > self.half_size.x {
      adjustment.x = -self.half_size.x;
    } else if point.x < -self.half_size.x {
      adjustment.x = self.half_size.x;
    }
    if point.y > self.half_size.y {
      adjustment.y = -self.half_size.y;
    } else if point.y < -self.half_size.y {
      adjustment.y = self.half_size.y;
    }
    2.0 * adjustment
  }

  fn get_wrapped_buckets(&self, ix: i32, iy: i32) -> Vec<(i32, i32)> {
    let right = (self.half_size.x / self.granularity).round() as i32;
    let top = (self.half_size.y / self.granularity).round() as i32;
    let left = (-self.half_size.x / self.granularity).round() as i32;
    let bottom = (-self.half_size.y / self.granularity).round() as i32;
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
    self
      .index
      .entry(self.bucket_coords(x, y))
      .or_default()
      .push(entity)
  }

  pub fn remove_entity(&mut self, entity: Entity, x_old: f32, y_old: f32) {
    let bucket_old = self
      .index
      .get_mut(&self.bucket_coords(x_old, y_old))
      .expect("no bucket");
    let idx = bucket_old
      .iter()
      .position(|&e| e == entity)
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

  pub fn get_entities(&self, (ix, iy): (i32, i32)) -> impl Iterator<Item = Entity> + Clone + '_ {
    Self::OFFSETS
      .iter()
      .cloned()
      .flat_map(move |xoff| {
        Self::OFFSETS
          .iter()
          .flat_map(move |&yoff| self.get_wrapped_buckets(ix + xoff, iy + yoff))
      })
      .flat_map(|offset| self.index.get(&offset))
      .flatten()
      .cloned()
  }

  pub fn get_entities_by_position(&self, x: f32, y: f32) -> impl Iterator<Item = Entity> + '_ {
    self.get_entities(self.bucket_coords(x, y))
  }

  pub fn bucket_coords(&self, x: f32, y: f32) -> (i32, i32) {
    (
      (x / self.granularity).round() as i32,
      (y / self.granularity).round() as i32,
    )
  }
}
