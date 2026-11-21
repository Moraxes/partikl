// Uniformy silnika (grupa 0, wiązanie 0); osobny moduł, żeby importowały je też reguły bez cyklu
// z `bindings.wesl`, który importuje typy reguł. Numer i nazwa muszą zgadzać się z `Binding::Uniforms`
// w `src/gpu/compute/bindings.rs`.

import super::types::EngineUniforms;

@group(0) @binding(0) var<uniform> uniforms: EngineUniforms;
