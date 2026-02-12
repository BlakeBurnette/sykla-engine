// Sykla cartoon cel-shading shader (WGSL)
// Quantizes light into discrete bands for a toon/cartoon look.

// This shader can be used as a post-processing effect or material shader.
// For MVP, we use Bevy's built-in StandardMaterial with bright, flat colors
// to approximate the cartoon aesthetic. This file is reserved for the
// full cel-shading implementation in a future phase.

// Example cel-shading fragment logic (for reference):
//
// fn cel_shade(normal: vec3<f32>, light_dir: vec3<f32>, base_color: vec4<f32>) -> vec4<f32> {
//     let ndotl = max(dot(normalize(normal), normalize(light_dir)), 0.0);
//
//     // Quantize to 3 bands
//     var shade: f32;
//     if ndotl > 0.7 {
//         shade = 1.0;
//     } else if ndotl > 0.3 {
//         shade = 0.6;
//     } else {
//         shade = 0.3;
//     }
//
//     return vec4<f32>(base_color.rgb * shade, base_color.a);
// }
