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
---
Cel (poziom B): step_driver importuje zawsze ten sam moduł rules; źródło to wbudowany default_rules, plik z --rules path.wesl (czytany z dysku, obserwowany) albo w M3 kod wygenerowany. Następny krok: sprawdzić rejestrację Shader::from_wesl pod stałą ścieżką modułu.
