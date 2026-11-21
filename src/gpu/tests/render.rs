use super::*;

/// Cztery nieruchome cząstki narysowane do obrazu 2× świata; piksele RGBA8 od lewego górnego rogu.
fn render_still_particles(quality: RenderQuality) -> Option<(Vec<u8>, UVec2, Vec2, [Vec2; 4])> {
  let (mut harness, size, world, positions) = still_particles_harness(quality)?;
  // Kilka klatek: zasoby (siatka, materiał, pipeline) trafiają na GPU z opóźnieniem.
  for _ in 0..6 {
    harness.update();
  }
  let pixels = harness.target_pixels();
  assert_eq!(pixels.len(), (size.x * size.y * 4) as usize);
  Some((pixels, size, world, positions))
}

/// Harness rysujący cztery nieruchome cząstki (jedna w środku obrazu) do obrazu 2× świata.
fn still_particles_harness(quality: RenderQuality) -> Option<(Harness, UVec2, Vec2, [Vec2; 4])> {
  use crate::gpu::headless::Offscreen;
  let world = Vec2::new(200.0, 120.0);
  let positions = [Vec2::new(0.0, 0.0), Vec2::new(-50.3, 20.7), Vec2::new(60.1, -30.9), Vec2::new(-80.0, -40.0)];
  let particles: Vec<GpuParticle> = positions
    .iter()
    .enumerate()
    .map(|(id, p)| GpuParticle { pos: p.to_array(), last: p.to_array(), species: 0, id: id as u32 })
    .collect();
  // Zerowe siły: cząstki stoją w miejscu.
  let spec = zero_forces(spec(&mut StdRng::seed_from_u64(1), 2));
  let size = (world * 2.0).as_uvec2();
  let harness = Harness::with_render(SimOptions::default(), spec, particles, world, Some(Offscreen { size, quality, app_colors: false, real_time: false }))?;
  Some((harness, size, world, positions))
}

/// Cząstki są rysowane tylko tam, gdzie leżą: na obrazie nie ma linii ani rozciągniętych
/// kształtów (błąd z `vertex_index` w `cb39173` rysował długie linie przez cały obraz).
fn assert_drawn_only_near_positions(quality: RenderQuality) {
  let Some((pixels, size, world, positions)) = render_still_particles(quality) else { return };
  // Piksel obrazu (od lewego górnego rogu) -> pozycja w świecie (y do góry).
  let world_of = |x: u32, y: u32| Vec2::new((x as f32 + 0.5) / 2.0 - world.x / 2.0, world.y / 2.0 - (y as f32 + 0.5) / 2.0);
  let mut lit_near = [0u32; 4];
  for y in 0..size.y {
    for x in 0..size.x {
      let i = ((y * size.x + x) * 4) as usize;
      // Tło jest czarne z szumem dithering (±1/255), cząstka jest wyraźnie jaśniejsza.
      let lit = pixels[i..i + 3].iter().any(|&c| c > 40);
      let w = world_of(x, y);
      let nearest = positions.iter().enumerate().map(|(k, p)| (k, p.distance(w))).min_by(|a, b| a.1.total_cmp(&b.1)).unwrap();
      // Promień cząstki to 2 jednostki; z marginesem na krawędź i wygładzanie.
      if lit {
        assert!(nearest.1 < 3.5, "zapalony piksel ({x}, {y}) {w} jest {} od najbliższej cząstki", nearest.1);
        lit_near[nearest.0] += 1;
      }
    }
  }
  assert!(lit_near.iter().all(|&n| n > 0), "cząstka bez zapalonych pikseli: {lit_near:?}");
}

#[test]
fn particles_are_drawn_only_near_their_positions() {
  assert_drawn_only_near_positions(RenderQuality::default());
}

#[test]
fn antialiased_particles_are_drawn_only_near_their_positions() {
  assert_drawn_only_near_positions(RenderQuality { analytic_aa: true, ..RenderQuality::default() });
}

/// Krawędź jest ostra (piksel albo jest cząstką, albo tłem) bez wygładzania i ma pośrednie
/// jasności z `analytic_aa`. MSAA prawie nie zmienia krawędzi elipsy (okrąg wycina `discard`).
#[test]
fn analytic_aa_smooths_the_edge_and_msaa_barely_does() {
  let intermediate = |quality| {
    render_still_particles(quality).map(|(pixels, ..)| pixels.chunks(4).filter(|p| (40..180).contains(&p[0])).count())
  };
  let base = RenderQuality::default();
  let (Some(sharp), Some(msaa), Some(smooth)) =
    (intermediate(base), intermediate(RenderQuality { msaa: Msaa::Sample4, ..base }), intermediate(RenderQuality { analytic_aa: true, ..base }))
  else {
    return;
  };
  assert_eq!(sharp, 0, "krawędź bez wygładzania ma pośrednie jasności");
  assert!(smooth > 0, "analityczne AA nie wygładziło krawędzi");
  // MSAA wygładza tylko miejsca, gdzie okrąg dotyka krawędzi quada (kilka pikseli na cząstkę),
  // a nie obwód elipsy.
  assert!(msaa * 10 < smooth, "MSAA wygładziło obwód elipsy: {msaa} pikseli wobec {smooth} z analitycznym AA");
}

/// Zmiana jakości w trakcie działania (klawisze B i A) działa na obraz: HDR tonemapuje (biały
/// ok. 196 zamiast 255), analityczne AA daje pośrednie jasności na krawędzi.
#[test]
fn render_quality_switches_at_runtime() {
  let base = RenderQuality::default();
  let Some((mut harness, size, ..)) = still_particles_harness(base) else { return };
  let frame = |harness: &mut Harness| {
    for _ in 0..4 {
      harness.update();
    }
    harness.target_pixels()
  };
  let centre = |p: &[u8]| p[((size.y / 2 * size.x + size.x / 2) * 4) as usize];
  let intermediate = |p: &[u8]| p.chunks(4).filter(|p| (40..180).contains(&p[0])).count();

  let ldr = frame(&mut harness);
  assert!(centre(&ldr) > 240, "LDR: biały powinien mieć 255, ma {}", centre(&ldr));
  assert_eq!(intermediate(&ldr), 0);

  harness.set_quality(RenderQuality { hdr: true, ..base });
  let hdr = frame(&mut harness);
  assert!(centre(&hdr) < 230, "HDR: biały powinien być tonemapowany, ma {}", centre(&hdr));

  harness.set_quality(base);
  assert!(centre(&frame(&mut harness)) > 240, "powrót do LDR");

  harness.set_quality(RenderQuality { analytic_aa: true, ..base });
  let smooth = frame(&mut harness);
  assert!(intermediate(&smooth) > 0, "analityczne AA nie wygładziło krawędzi po włączeniu w trakcie");
  assert!(centre(&smooth) > 240);

  harness.set_quality(base);
  assert_eq!(intermediate(&frame(&mut harness)), 0, "krawędź nadal wygładzona po wyłączeniu AA");
}
