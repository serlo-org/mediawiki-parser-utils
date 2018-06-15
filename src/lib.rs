//! This library provides common, Mathe-für-Nicht-Freaks specific code.

extern crate mediawiki_parser;
#[macro_use]
extern crate serde_derive;

#[macro_use]
mod util;
pub mod transformations;

pub use util::*;
