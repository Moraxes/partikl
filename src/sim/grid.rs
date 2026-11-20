use bevy::prelude::*;

use super::InteractionId;

/// Siatka przestrzenna z zawijaniem (torus) do szukania sąsiadów cząstek.
/// Świat obejmuje `[-half_size, half_size]` w obu osiach.
///
/// Komórka `(ix, iy)` zawiera pozycje, dla których `round(x / granularity) == ix` (tak samo
/// dla y). Skrajne komórki (`left`/`right`, `bottom`/`top`) leżą w połowie poza światem i
/// razem tworzą jedną komórkę na szwie, dlatego zapytanie o komórkę przy krawędzi zagląda też
/// do jej odpowiednika po drugiej stronie. Komórki są w płaskim `Vec`, wierszami.
///
/// Cząstki leżą w tablicach posortowanych według komórek (encja, x, y, typ): komórka `c` to
/// zakres `cell_range(c)`. Siatka nie jest aktualizowana przyrostowo: `rebuild` co krok
/// układa wszystkie cząstki od nowa sortowaniem przez zliczanie (dwa przejścia), więc pozycje
/// są zawsze aktualne, a liczenie sił czyta je wprost, bez pytania ECS o każdą parę.
#[derive(Resource)]
pub struct SpatialGrid {
  half_size: Vec2,
  granularity: f32,
  left: i32,
  right: i32,
  bottom: i32,
  top: i32,
  columns: usize,
  cell_count: usize,
  /// Początek komórki `c` w tablicach cząstek to `starts[c]`, koniec `starts[c + 1]`.
  starts: Vec<u32>,
  entities: Vec<Entity>,
  /// Pozycje z zapasem `PADDING` wartości NaN na końcu (odczyty wektorowe poza ostatnią
  /// cząstką; NaN nigdy nie jest w zasięgu).
  xs: Vec<f32>,
  ys: Vec<f32>,
  types: Vec<InteractionId>,
  /// Bufory `rebuild` używane ponownie między krokami.
  staged: Vec<Staged>,
  cursor: Vec<u32>,
  /// `placed[k]`: miejsce w tablicach `k`-tej cząstki podanej ostatniemu `rebuild`.
  placed: Vec<u32>,
  /// Sąsiedztwo każdej komórki policzone raz w `new`: indeksy komórek do przejrzenia (z
  /// zawijaniem) dla komórki `c` to `neighbour_cells[neighbour_starts[c]..neighbour_starts[c + 1]]`.
  neighbour_starts: Vec<u32>,
  neighbour_cells: Vec<u32>,
  /// To samo sąsiedztwo jako posortowane przebiegi kolejnych komórek (`NeighbourRun`): komórki
  /// leżą wierszami, więc trzy komórki jednego wiersza to zwykle jeden przebieg. Dla komórki
  /// `c`: `neighbour_runs[run_starts[c]..run_starts[c + 1]]`.
  run_starts: Vec<u32>,
  neighbour_runs: Vec<NeighbourRun>,
}

/// Kolejne komórki `first..last` z sąsiedztwa jednej komórki, wszystkie po tej samej stronie
/// szwu. `shift` (0 albo ± rozmiar świata w każdej osi) przenosi pozycje z tych komórek do
/// obrazu najbliższego pytającej komórce: `(pozycja − pytająca) + shift` daje ten sam wynik co
/// `get_corrected_position_delta`, bez porównań dla każdej pary.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NeighbourRun {
  pub first: u32,
  pub last: u32,
  pub shift: Vec2,
}

/// Cząstka przed rozłożeniem do komórek w `rebuild`.
#[derive(Clone, Copy)]
struct Staged {
  cell: u32,
  entity: Entity,
  position: Vec2,
  interaction: InteractionId,
}

/// Cząstka w komórce siatki (kopia danych z tablic).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Neighbour {
  pub entity: Entity,
  pub position: Vec2,
  pub interaction: InteractionId,
}

/// Komórki do przejrzenia dla jednej komórki sąsiedztwa: ona sama i do trzech odpowiedników
/// po drugiej stronie szwu. `len` mówi, ile pozycji tablicy jest ważnych.
#[derive(Clone, Copy)]
struct Wrapped {
  buckets: [(i32, i32); 4],
  len: usize,
}

impl SpatialGrid {
  /// Zapas na końcu `xs()` i `ys()` (wartości NaN), żeby odczyt wektora do 8 liczb od
  /// dowolnej cząstki nie wychodził poza tablicę.
  pub const PADDING: usize = 8;

  const OFFSETS: [(i32, i32); 9] = [
    (-1, -1),
    (-1, 0),
    (-1, 1),
    (0, -1),
    (0, 0),
    (0, 1),
    (1, -1),
    (1, 0),
    (1, 1),
  ];

  pub fn new(width: f32, height: f32, granularity: f32) -> SpatialGrid {
    let half_size = Vec2::new(width / 2.0, height / 2.0);
    let right = (half_size.x / granularity).round() as i32;
    let top = (half_size.y / granularity).round() as i32;
    let left = (-half_size.x / granularity).round() as i32;
    let bottom = (-half_size.y / granularity).round() as i32;
    let columns = (right - left + 1) as usize;
    let rows = (top - bottom + 1) as usize;
    let mut grid = SpatialGrid {
      half_size,
      granularity,
      left,
      right,
      bottom,
      top,
      columns,
      cell_count: columns * rows,
      starts: vec![0; columns * rows + 1],
      entities: Vec::new(),
      xs: vec![f32::NAN; Self::PADDING],
      ys: vec![f32::NAN; Self::PADDING],
      types: Vec::new(),
      staged: Vec::new(),
      cursor: Vec::new(),
      placed: Vec::new(),
      neighbour_starts: Vec::new(),
      neighbour_cells: Vec::new(),
      run_starts: Vec::new(),
      neighbour_runs: Vec::new(),
    };
    let mut starts = Vec::with_capacity(columns * rows + 1);
    let mut cells = Vec::with_capacity(columns * rows * 9 + 64);
    starts.push(0);
    for iy in bottom..=top {
      for ix in left..=right {
        cells.extend(grid.neighbour_cells_of((ix, iy)).map(|c| c as u32));
        starts.push(cells.len() as u32);
      }
    }
    grid.neighbour_starts = starts;
    grid.neighbour_cells = cells;

    let mut run_starts = Vec::with_capacity(columns * rows + 1);
    let mut runs: Vec<NeighbourRun> = Vec::with_capacity(columns * rows * 3 + 64);
    let mut sorted = Vec::with_capacity(36);
    run_starts.push(0);
    for iy in bottom..=top {
      for ix in left..=right {
        sorted.clear();
        sorted.extend(grid.shifted_neighbour_cells_of((ix, iy)));
        sorted.sort_unstable_by_key(|&(cell, _)| cell);
        let first = runs.len();
        for &(cell, shift) in &sorted {
          let cell = cell as u32;
          match runs[first..].last_mut() {
            Some(run) if run.last == cell && run.shift == shift => run.last += 1,
            _ => runs.push(NeighbourRun { first: cell, last: cell + 1, shift }),
          }
        }
        run_starts.push(runs.len() as u32);
      }
    }
    grid.run_starts = run_starts;
    grid.neighbour_runs = runs;
    grid
  }

  /// Indeksy komórek sąsiedztwa `(ix, iy)` z zawijaniem, w stałej kolejności (`OFFSETS`).
  fn neighbour_cells_of(&self, coords: (i32, i32)) -> impl Iterator<Item = usize> + Clone + '_ {
    self.shifted_neighbour_cells_of(coords).map(|(cell, _)| cell)
  }

  /// Jak `neighbour_cells_of`, z przesunięciem pozycji w komórce do obrazu po stronie
  /// pytającej (zob. `NeighbourRun::shift`): komórka zawinięta w prawo leży logicznie po lewej.
  fn shifted_neighbour_cells_of(&self, (ix, iy): (i32, i32)) -> impl Iterator<Item = (usize, Vec2)> + Clone + '_ {
    let size = self.half_size + self.half_size;
    let shift = |moved: i32, size: f32| match moved.signum() {
      1 => -size,
      -1 => size,
      _ => 0.0,
    };
    Self::OFFSETS
      .iter()
      .flat_map(move |&(xoff, yoff)| {
        let logical = (ix + xoff, iy + yoff);
        let wrapped = self.get_wrapped_buckets(logical.0, logical.1);
        wrapped.buckets.into_iter().take(wrapped.len).map(move |bucket| {
          let moved = Vec2::new(shift(bucket.0 - logical.0, size.x), shift(bucket.1 - logical.1, size.y));
          (bucket, moved)
        })
      })
      .filter_map(|(bucket, moved)| Some((self.cell_index(bucket)?, moved)))
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

  fn get_wrapped_buckets(&self, ix: i32, iy: i32) -> Wrapped {
    let (left, right, bottom, top) = (self.left, self.right, self.bottom, self.top);
    let mut buckets = [(ix, iy); 4];
    let mut len = 1;

    if ix <= left {
      buckets[len].0 += right - left;
      len += 1;
    } else if ix >= right {
      buckets[len].0 -= right - left;
      len += 1;
    }

    if iy <= bottom {
      buckets[len].1 += top - bottom;
      len += 1;
    } else if iy >= top {
      buckets[len].1 -= top - bottom;
      len += 1;
    }

    if (ix <= left || ix >= right) && (iy <= bottom || iy >= top) {
      buckets[len].0 = buckets[1].0;
      buckets[len].1 = buckets[2].1;
      len += 1;
    }

    Wrapped { buckets, len }
  }

  #[cfg(test)]
  fn neighbour_cells_at(&self, c: usize) -> &[u32] {
    &self.neighbour_cells[self.neighbour_starts[c] as usize..self.neighbour_starts[c + 1] as usize]
  }

  pub fn half_size(&self) -> Vec2 {
    self.half_size
  }

  /// Liczba komórek (indeksy `0..cell_count()`, wierszami).
  pub fn cell_count(&self) -> usize {
    self.cell_count
  }

  /// Zakres cząstek komórki `c` w tablicach (`entities`, `xs`, `ys`, `types`).
  pub fn cell_range(&self, c: usize) -> std::ops::Range<usize> {
    self.starts[c] as usize..self.starts[c + 1] as usize
  }

  /// Początki komórek: komórka `c` to `starts[c]..starts[c + 1]`; długość `cell_count() + 1`.
  pub fn cell_starts(&self) -> &[u32] {
    &self.starts
  }

  /// Liczba cząstek w siatce.
  pub fn len(&self) -> usize {
    self.entities.len()
  }

  pub fn is_empty(&self) -> bool {
    self.entities.is_empty()
  }

  pub fn entities(&self) -> &[Entity] {
    &self.entities
  }

  /// Współrzędne x w kolejności komórek, z zapasem `PADDING` (NaN) na końcu.
  pub fn xs(&self) -> &[f32] {
    &self.xs
  }

  /// Współrzędne y, jak `xs`.
  pub fn ys(&self) -> &[f32] {
    &self.ys
  }

  pub fn types(&self) -> &[InteractionId] {
    &self.types
  }

  /// Miejsce w tablicach każdej cząstki w kolejności podania ostatniemu `rebuild`: `k`-ta
  /// podana leży pod indeksem `placed()[k]`. Pozwala przepisać wyniki liczone w kolejności
  /// siatki z powrotem do ECS przy iteracji w tej samej kolejności co przy budowaniu.
  pub fn placed(&self) -> &[u32] {
    &self.placed
  }

  fn neighbour_at(&self, i: usize) -> Neighbour {
    Neighbour { entity: self.entities[i], position: Vec2::new(self.xs[i], self.ys[i]), interaction: self.types[i] }
  }

  /// Indeks komórki zawierającej pozycję ze świata.
  pub fn cell_at(&self, x: f32, y: f32) -> usize {
    self
      .cell_index(self.bucket_coords(x, y))
      .unwrap_or_else(|| panic!("position ({x}, {y}) outside the world"))
  }

  /// Sąsiedztwo komórki `c` jako przebiegi kolejnych komórek, rosnąco. Te same komórki co w
  /// `get_neighbour_cells`, w innej kolejności.
  pub fn neighbour_runs(&self, c: usize) -> &[NeighbourRun] {
    &self.neighbour_runs[self.run_starts[c] as usize..self.run_starts[c + 1] as usize]
  }

  /// Indeks komórki (wierszami), albo `None` poza siatką (taka komórka jest zawsze pusta).
  fn cell_index(&self, (ix, iy): (i32, i32)) -> Option<usize> {
    if ix < self.left || ix > self.right || iy < self.bottom || iy > self.top {
      return None;
    }
    Some((iy - self.bottom) as usize * self.columns + (ix - self.left) as usize)
  }


  /// Układa siatkę od nowa z podanych cząstek (poprzednia zawartość znika): indeks komórki
  /// każdej cząstki, zliczanie, suma prefiksowa i rozłożenie do tablic. W komórce cząstki leżą
  /// w kolejności podania. Panika, jeśli pozycja jest poza światem.
  pub fn rebuild(&mut self, particles: impl IntoIterator<Item = (Entity, Vec2, InteractionId)>) {
    let mut staged = std::mem::take(&mut self.staged);
    staged.clear();
    let mut counts = std::mem::take(&mut self.cursor);
    counts.clear();
    counts.resize(self.cell_count, 0);
    for (entity, position, interaction) in particles {
      let cell = self.cell_at(position.x, position.y) as u32;
      counts[cell as usize] += 1;
      staged.push(Staged { cell, entity, position, interaction });
    }

    // `starts` z liczności, `counts` staje się kursorem zapisu każdej komórki.
    let mut total = 0;
    for (start, count) in self.starts.iter_mut().zip(counts.iter_mut()) {
      *start = total;
      total += *count;
      *count = *start;
    }
    self.starts[self.cell_count] = total;

    let n = staged.len();
    let placeholder = Entity::PLACEHOLDER;
    self.entities.clear();
    self.entities.resize(n, placeholder);
    self.types.clear();
    self.types.resize(n, InteractionId(0));
    self.xs.resize(n + Self::PADDING, 0.0);
    self.ys.resize(n + Self::PADDING, 0.0);
    self.xs[n..].fill(f32::NAN);
    self.ys[n..].fill(f32::NAN);
    self.placed.clear();
    for p in &staged {
      let i = counts[p.cell as usize] as usize;
      counts[p.cell as usize] += 1;
      self.placed.push(i as u32);
      self.entities[i] = p.entity;
      self.xs[i] = p.position.x;
      self.ys[i] = p.position.y;
      self.types[i] = p.interaction;
    }
    self.staged = staged;
    self.cursor = counts;
  }

  /// Cząstki z komórki `(ix, iy)` i ośmiu sąsiednich, z zawijaniem. Zawiera też cząstkę, o
  /// którą pytamy, jeśli leży w tej komórce.
  ///
  /// Dla komórki w siatce sąsiedztwo jest z tablicy z `new`; poza siatką liczone na miejscu.
  pub fn get_neighbours(&self, coords: (i32, i32)) -> impl Iterator<Item = Neighbour> + Clone + '_ {
    self.get_neighbour_cells(coords).flatten().map(|i| self.neighbour_at(i))
  }

  /// Jak `get_neighbours`, ale po komórkach: każda jako zakres w tablicach, w tej samej
  /// kolejności.
  pub fn get_neighbour_cells(
    &self,
    coords: (i32, i32),
  ) -> impl Iterator<Item = std::ops::Range<usize>> + Clone + '_ {
    let (precomputed, computed) = match self.cell_index(coords) {
      Some(c) => {
        let range = self.neighbour_starts[c] as usize..self.neighbour_starts[c + 1] as usize;
        (Some(self.neighbour_cells[range].iter().map(|&i| i as usize)), None)
      }
      None => (None, Some(self.neighbour_cells_of(coords))),
    };
    precomputed
      .into_iter()
      .flatten()
      .chain(computed.into_iter().flatten())
      .map(|index| self.cell_range(index))
  }

  pub fn get_neighbours_by_position(&self, x: f32, y: f32) -> impl Iterator<Item = Neighbour> + '_ {
    self.get_neighbours(self.bucket_coords(x, y))
  }

  pub fn get_entities(&self, coords: (i32, i32)) -> impl Iterator<Item = Entity> + Clone + '_ {
    self.get_neighbours(coords).map(|n| n.entity)
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

#[cfg(test)]
mod tests;
