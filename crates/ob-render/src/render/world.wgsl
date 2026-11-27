#include "world_prelude.wgsl"
#include "native/world/common.wgsl"
#if TEXTURE_BINDING_PLAIN
#include "native/world/bind_plain.wgsl"
#else
#include "native/world/bind_array.wgsl"
#endif
@group(2) @binding(0) var reflected_tex: texture_2d<f32>;
@group(2) @binding(1) var reflected_samp: sampler;
// skymorph.efx only: SKYMORPHTEXTURE, the sky pair faded from.
@group(3) @binding(0) var morph_tex: texture_2d<f32>;
@group(3) @binding(1) var morph_samp: sampler;
#if LAMPS
// BEGIN LAMPS
#include "enh/lighting/lamps.wgsl"
#if MODERN
// BEGIN MODERN
#include "enh/lighting/modern.wgsl"
// END MODERN
#endif
#include "enh/lighting/beams.wgsl"
#include "enh/lighting/world_lit.wgsl"
#include "enh/lighting/shadow_depth.wgsl"
// END LAMPS
#endif
#include "native/world/vertex.wgsl"
#include "enh/geometry/world_fade.wgsl"
#include "native/world/fragment.wgsl"
#if LAMPS
// BEGIN LAMPS
#include "enh/weather/rain_lit.wgsl"
// END LAMPS
#endif
#include "enh/materials/world_reflect_enh.wgsl"
#include "native/world/fragment_reflect.wgsl"
#include "enh/weather/wetness.wgsl"
#include "enh/materials/world_wet.wgsl"
#include "enh/geometry/world_masks.wgsl"
#include "native/world/fragment_sky.wgsl"
#include "enh/geometry/world_coverage.wgsl"
#include "wet_road_world.wgsl"
