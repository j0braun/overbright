//! `WorldRenderer::new` cut into its build stages (audit plan render
//! §2). `new` runs them in this order, each taking what the earlier ones
//! made, so the texture slots, the draw order and the GPU objects come out
//! as before: [`geometry::prepare_geometry`], [`geometry::build_groups`],
//! [`textures::assign_textures`] (the tables' slots first),
//! [`draws::build_draws`] (the draws' slots in group order, then the
//! sort), [`textures::classify_surfaces`], [`textures::upload_textures`],
//! [`buffers::make_buffers`], [`buffers::make_frames`],
//! [`buffers::make_reflection_binding`], [`buffers::make_pipelines`],
//! [`geometry::build_bounds`].

pub(in crate::render) mod buffers;
pub(in crate::render) mod draws;
pub(in crate::render) mod geometry;
pub(in crate::render) mod textures;
