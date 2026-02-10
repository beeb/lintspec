#![cfg_attr(docsrs, feature(doc_cfg))]
//! Core library for lintspec.
//!
//! This crate provides the core parsing and validation logic for lintspec.
pub mod config;
pub mod definitions;
pub mod error;
pub mod files;
pub mod lint;
pub mod natspec;
pub mod parser;
pub(crate) mod prelude;
pub mod textindex;

#[cfg_attr(docsrs, doc(cfg(feature = "slang")))]
#[cfg(feature = "slang")]
pub mod utils;
