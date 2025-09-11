#![allow(clippy::doc_markdown)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod config;
pub mod definitions;
pub mod error;
pub mod files;
pub mod lint;
pub mod natspec;
pub mod parser;

#[cfg_attr(docsrs, doc(cfg(feature = "cli")))]
#[cfg(feature = "cli")]
pub mod cli;

#[cfg_attr(docsrs, doc(cfg(feature = "slang")))]
#[cfg(feature = "slang")]
pub mod utils;
