//! Darn chunker — port of cashewe/darn (MIT).
//! See `LICENSE-DARN` in this directory.

mod chunk_optimiser;
mod chunker;
mod md_parser;
mod rule_manager;

pub use chunker::DarnChunker;
