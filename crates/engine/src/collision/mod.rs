//! Collision: run-time structures built from solids' collision meshes, trees and convex volumes, and the queries
//! gameplay needs (specs/sly1/collision.md, classes/solid-object.md, classes/shared-structures.md).
//!
//! | Module | What |
//! |---|---|
//! | [`geom`] | boxes, triangles, closest points, ray / sphere / capsule tests against one triangle |
//! | [`bvh`] | the bounding-volume hierarchy used over triangles and over bodies |
//! | [`mesh`] | one collision mesh ready for queries: triangle planes, hierarchy, edges, ledges, the collision tree |
//! | [`world`] | shapes placed as bodies, world-space ray casts, sweeps, overlaps and ground probes |
//! | [`volume`] | the convex-volume inside test and placed volumes |
//! | [`material`] | friction and elasticity between materials |

pub mod bvh;
pub mod geom;
pub mod material;
pub mod mesh;
pub mod volume;
pub mod world;

pub use material::Materials;
pub use mesh::{Ledge, TriMesh};
pub use volume::{PlacedVolume, point_in_convex, world_volumes};
pub use world::{BodyId, CollisionWorld, GroundHit, Overlap, QueryFilter, RayHit, Shape, ShapeId, SweepHit};
