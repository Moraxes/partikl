---
id: 78
title: Reguły domyślne jako węzły
summary: default_rules przepisane na ręczne węzły o jednolitych sygnaturach
status: todo
priority: soon
branch: main
blocked_by:
- 77
created: 2026-11-22T18:37:33
---
Cel: default_rules jako małe funkcje WESL (jądra odległości, bramki, wkłady do akumulatorów, post-process po akumulacji, całkowanie) o jednolitych sygnaturach: zaczątek M3 (węzły, potem format grafu i codegen). Domeny pól: para, cząstka, globalna; Accumulate z trybem sum/mean/capped_mean(N0) i własnym licznikiem, więc AVERAGE_FORCES staje się capped_mean na klocku przyciągania (czy obejmuje sprężynę: do decyzji); siły spoza pętli par po akumulacji, bez skalowania. Wynik bit w bit lub w tolerancji wyroczni; A/B kosztu dodatkowych akumulatorów. Claude prowadzi użytkownika.
