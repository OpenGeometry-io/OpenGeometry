#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable
    )
)]
#![allow(clippy::needless_range_loop, clippy::type_complexity)]
#![deny(
    unreachable_pub,
    clippy::wildcard_imports,
    clippy::todo,
    clippy::unimplemented,
    clippy::dbg_macro,
    clippy::iter_over_hash_type
)]

pub mod math;

pub mod brep;

pub(crate) mod geom2d;

pub mod query;

pub mod primitives;
pub mod tessellation;

pub(crate) mod intersection;

pub mod operations;

pub mod exchange;

pub mod world_graph;

pub mod bindings;
