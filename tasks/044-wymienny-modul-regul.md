---
id: 44
title: Wymienny moduł reguł
summary: Stały moduł rules, którego źródło wybiera Rust (wbudowany, --rules plik, M3 codegen)
status: doing
priority: soon
branch: main
part_of: 35
blocked_by:
- 43
created: 2026-11-21T12:39:03
worked: 2026-11-21T23:43:29
edited: 2026-11-22T00:04:23
---
Cel (poziom B): step_driver importuje zawsze ten sam moduł rules; źródło to wbudowany default_rules, plik z --rules path.wesl (czytany z dysku, obserwowany) albo w M3 kod wygenerowany. Następny krok: sprawdzić rejestrację Shader::from_wesl pod stałą ścieżką modułu.


Wynik: step_driver importuje stały moduł `rules::model` (shader `embedded://partikl/gpu/shaders/rules/model.wesl`, wkładany do Assets<Shader> synchronicznie w RulesPlugin::build, więc bez wyścigu ładowania). Źródło: zasób `RuleSource` {Builtin, Diagnostic, File(path)}; `--rules path.wesl` (czytany przy starcie, błąd = komunikat i wyjście), plik odpytywany co 0,5 s i podmieniany w Assets<Shader> (pipeline'y się rekompilują); zły plik zostawia poprzedni moduł, błąd kompilacji tylko w logu. RuleSet i flaga RULES_DIAGNOSTIC usunięte, ForcesVariant bez reguł. Importy w źródle względem shaders/rules/, więc default_rules.wesl działa jako plik wprost. Wbudowane reguły przez include_str!, nie przez load_shader_library (hot reload default_rules z feature dev zastępuje --rules). Testy: plik = wbudowane bit w bit, przeładowanie po zapisie, nieczytelny plik. Commit 44b107b, 55 testów OK. Do 049: moduł reguł nie ma już wyścigu, matrix.wesl nadal ładowany asynchronicznie.
