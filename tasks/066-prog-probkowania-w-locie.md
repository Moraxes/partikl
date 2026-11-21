---
id: 66
title: Próg próbkowania w locie
summary: max_candidates jako zasób sterowany w runtime (klawisz, HUD), opcjonalnie adaptacyjny z StepBudget
status: todo
priority: later
branch: compute-shader-1
part_of: 54
created: 2026-11-21T19:25:55
---
Cel: sterować progiem próbkowania sąsiadów (dziś stała MAX_CANDIDATES = 512 w src/gpu/compute/mod.rs, przekazywana co krok w SimUniforms::max_candidates, więc zmiana działa od następnego kroku, bez przekompilowania shadera). Przy dużych n koszt kroku ∝ min(kandydaci, próg): przy 500k–1M jednostajnych (2400–4800 kandydatów) próg 256 daje ok. 2× szybszy przebieg sił, kosztem większej wariancji sił (losowe próbkowanie z wagą 'every'). Zakres: zasób (np. SamplingCap, wyciągany do świata renderowania jak SimOptions), klawisz przełączający wartości (np. 128/256/512/1024), linia w HUD, flaga startowa; opcjonalnie tryb adaptacyjny: gdy krok przekracza budżet (StepBudget z 059), najpierw obniżać próg zamiast zwalniać symulację (albo odwrotnie; do wyboru). Wymaga akceptacji wizualnej użytkownika w ruchu przed zmianą domyślnego (galeria: bench render --gallery). Priorytet: odłożone na życzenie użytkownika (2026-11-21), na razie nie robić. Następny krok: gdy użytkownik zdecyduje, zasób + klawisz + test kroku dla kilku wartości progu.
