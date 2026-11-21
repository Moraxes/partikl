---
id: 62
title: Matematyka par f16
summary: 'Ścieżka par w f16 (jednostki F16 bezczynne, F32 na 84% limitera): wariant shadera, dokładność, sąsiedzi 8 B'
status: todo
priority: later
branch: compute-shader-1
part_of: 54
relates:
- 58
- 60
created: 2026-11-21T19:19:16
edited: 2026-11-21T19:34:53
---
Cel: odciążyć jednostki F32 (limiter 84%) przez matematykę par w f16; profil liczników (061) pokazuje F16 bezczynne. Spike (2026-11-21, M5, bench gpu --quick, przebieg sił µs uniform/clusters/dense, baza 375/525/560): (1) SHADER_F16 jest na adapterze i już włączony na urządzeniu (Bevy domyślnie włącza wszystkie funkcje adaptera); (2) w WESL 'enable f16;' musi stać PO liniach import (przed importem błąd parsera), z tym kompiluje się przez Bevy/naga/Metal i testy przechodzą; (3) częściowy f16 z konwersjami nie pomaga: jądro przyciągania w f16 372/523/556 (±0), + inverseSqrt w f16 384/537/574 (+2…+3%); (4) pełna ścieżka par w f16 (delta konwertowana z f32 raz na kandydata, cała matematyka i akumulacja w f16, spring+average): 339/464/493 (−10…−12%), bez dokładności i bez adaptive/resolve. Wniosek: opłaca się tylko cała ścieżka bez konwersji w pętli. Ryzyka: precyzja akumulacji w f16 (do ±65504, błąd względny 2^-11 na dodawanie), jednostka integer/conditional też na 74% limitera (rozpakowanie f16 z u32 to operacje całkowite), nieznane, czy F16 i F32 wydają się równolegle. Kolejność: wariant shadera (bramka: ≥8% bez regresji dokładności) → dokładność i decyzja o domyślnym → sąsiedzi 8 B (bramka: ≥ dalsze 8%). Następny krok: podzadania (tq tree).

Polityka zatwierdzania (2026-11-21, użytkownik): domyślne f32; f16 przełączane w locie klawiszem i flagą startową (patrz 063). Zmiana domyślnego na f16 wymaga akceptacji wizualnej użytkownika w ruchu, chyba że zysk wydajnościowy jest znaczny (> 25% szybszy przebieg sił). Spike dał −10…−12% (pełna ścieżka bez konwersji sąsiadów), więc prawdopodobnie bez zatwierdzenia nie wejdzie jako domyślne; dojście do > 25% wymagałoby zadania 065 (sąsiedzi 8 B).

Zmiana (2026-11-21, użytkownik): f16 jako FLAGA STARTOWA (--f16), bez klawisza i przełączania w locie; porównanie w ruchu przez dwa uruchomienia. Zakres spike'u rozszerzony: nie tylko siły, ale cały potok (pozycje i prędkości w f16), patrz wyniki spike'u w 067.
