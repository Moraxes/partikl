---
id: 67
title: Spike f16 potok
summary: 'Pozycje i prędkości f16 oraz sąsiedzi 8 B f16: numerycznie tak, wydajnościowo nie'
status: done
priority: soon
branch: compute-shader-1
part_of: 62
created: 2026-11-21T19:57:41
worked: 2026-11-21T19:57:41
---
Wynik spike'u (2026-11-21, M5, wszystkie ablacje tymczasowe, kod przywrócony). A. Numerycznie, stan ustalony (8000 cząstek, macierz seed 0, 3 typy, v² ≈ 2800, przesunięcie ok. 0,5 jedn./krok), 1000 kroków od tego samego stanu, v² / odsetek zatrzymanych cząstek (<0,5 jedn./s): f32 2700–2810 / 0%; pozycje BEZWZGLĘDNE w f16 (pos i last) 3320–3460 (+20…23% energii kinetycznej) / 0,4–0,5%; pozycje w f16 względem komórki (pos − początek komórki) 2700–2860 / 0%; prędkość (pos − last) w f16 2720–2820 / 0%; liczba sąsiadów w 20 jedn. bez różnic. Wniosek: pozycje bezwzględne w f16 odpadają (świat ±900 ⇒ rozdzielczość 0,5 jedn. przy |x| > 512, a test miał świat 500×330, więc w prawdziwym byłoby ok. 4× gorzej); względem komórki i prędkość w f16 są fizycznie nieodróżnialne od f32. B. Wydajność (przebieg sił, µs; 100k uniform/clusters/dense, baza 375/525/560; 500k uniform/clusters, baza 2605/2746): pełna ścieżka par w f16 z konwersją delty z f32 (spike S3) 339/464/493 (−10…−12%); prototyp 'sąsiedzi 8 B' (struct {pos: vec2<f16> względem komórki, packed: u32 z indeksem komórki i typem}, pętla po 25 komórkach z bazą f16 liczoną raz na komórkę, matematyka i akumulacja w f16 bez konwersji w pętli, odczyty ×4 naprzód): 494/588/595 przy 100k (+32/+12/+6%) i 2862/2932 przy 500k (+10/+7%), błąd pozycji po kroku 9e-4 względem wyroczni f64. Czyli restrukturyzacja do pętli po komórkach (25 × odczyt przesunięcia i konwersja bazy, krótkie pętle) kosztuje więcej, niż daje f16. C. Ograniczenia toolchainu: WESL 'enable f16;' po importach; naga nie przyjmuje bitcast<u32>(vec2<f16>) ani bitcast<vec2<f16>>(u32) (użyć struktury z polem vec2<f16> albo pack/unpack2x16float); 'meta' to słowo zarezerwowane. Wniosek: w zasięgu jest co najwyżej ok. 10–12% (S3), z kosztem precyzji akumulacji w f16 i utrzymania, a próg użytkownika dla zmiany domyślnego bez akceptacji wizualnej to > 25%; stan w f16 niczego nie przyspiesza (pamięć nie jest wąskim gardłem, L1 2,6%). Rekomendacja: zadanie 065 zamknąć (nieopłacalne), 063/064 odłożyć (later), wrócić do f16 tylko gdy profil znów pokaże F32 jako jedyne wąskie gardło i skończą się tańsze kierunki.
