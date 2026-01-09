#![allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    unnecessary_transmutes,
    clippy::approx_constant,
    clippy::missing_safety_doc,
    clippy::ptr_offset_with_cast
)]

mod error;
mod raii;

pub use error::*;
pub use raii::*;

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
