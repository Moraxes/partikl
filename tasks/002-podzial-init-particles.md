---
id: 2
title: Podział init_particles
summary: Spawn, siatka i kamera osobno; rozmiar świata z konfiguracji, nie z okna
status: done
priority: soon
branch: main
created: 2026-11-20T18:44:47
worked: 2026-11-21T22:43:06
edited: 2026-11-21T22:43:15
---
`init_particles` w `src/render.rs` robi naraz spawn cząstek z gizmami, budowę `SpatialGrid` i spawn kamery, a rozmiar świata bierze z okna (symulacja zależy od rozdzielczości). Następny krok: rozdzielić na osobne systemy, rozmiar świata przenieść do konfiguracji (`config.rs` albo zasób).

## Szczegóły
- Kamera, zoom i pan: wyjąć `MainCamera` i `handle_mouse_input` (zoom, pan, kursor) do `render/camera.rs`. Z `ui/mod.rs` zostaje reszta wejścia.
- `ui/selection.rs::select_on_click` (~70 linii): zastąpić ręczną macierz kamery przez `Camera::viewport_to_world_2d`, usunąć zbędne pole `translation` z `SelectedGizmo`. Zależność `render → ui::selection` (markery Selection/Highlight) odwrócić albo przenieść markery do wspólnego miejsca.
- Sprawdzić logikę `CursorOptions` w `handle_mouse_input`: wygląda na odwróconą (Locked + visible po puszczeniu LMB, None + hidden po wciśnięciu z Ctrl).
- Brakuje `assets/FiraMono-Regular.ttf` (błąd ładowania fontu przy starcie); ustalić, czy to zamierzone.

## Testy czekające na naprawę
Zamierzone zachowanie opisują testy z `#[ignore]` (`cargo test -- --ignored` pokazuje, że failują): `click_selects_particle_under_cursor_when_zoomed` (zoom ignorowany w `select_on_click`) i `click_selects_particle_under_cursor_when_camera_is_panned_in_y` (odwrócony znak y przy przeliczaniu kursora). Po naprawie usunąć `#[ignore]`. Logika `CursorOptions` nie ma testu: zamierzone zachowanie trzeba najpierw ustalić, potem napisać test.

Zamknięte (2026-11-21, review backlogu): init_particles, select_on_click i testy #[ignore] zniknęły ze ścieżką CPU; font jest w assets. Przeciąganie kamery lewym przyciskiem z kursorem blokowanym tylko podczas przeciągania naprawione i przetestowane (commit bea5125). Rozmiar świata z ekranu to świadoma decyzja (ScreenSize).
