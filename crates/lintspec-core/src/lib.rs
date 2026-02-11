#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!(concat!("../", std::env!("CARGO_PKG_README")))]
pub mod config;
pub mod definitions;
pub mod error;
pub mod files;
pub mod interner;
pub mod lint;
pub mod natspec;
pub mod parser;
pub(crate) mod prelude;
pub mod textindex;

#[cfg_attr(docsrs, doc(cfg(feature = "slang")))]
#[cfg(feature = "slang")]
pub mod utils;
