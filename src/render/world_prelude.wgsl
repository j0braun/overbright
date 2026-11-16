#if LAMPS
#if MODERN
#if MLRT
enable wgpu_ray_query;
#endif
#endif
#endif
#if TEXTURE_BINDING_BOUND
enable wgpu_binding_array;
#endif
#if MIP_BIAS
override MIP_BIAS: f32 = 0.0;
#endif
