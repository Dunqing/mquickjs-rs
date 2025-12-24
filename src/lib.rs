//! MQuickJS - A Rust port of Fabrice Bellard's MicroQuickJS JavaScript engine
//!
//! MQuickJS is a minimalist JavaScript engine targeting embedded systems.
//! It can run JS programs with as low as 10KB of RAM.
//!
//! # Features
//! - Subset of ES5 JavaScript with "stricter mode"
//! - Tracing and compacting garbage collector
//! - Stack-based bytecode VM
//! - UTF-8 string storage
//! - No external dependencies for core functionality
//!
//! # Example
//! ```ignore
//! use mquickjs::{Context, Value};
//!
//! let mut ctx = Context::new(64 * 1024); // 64KB memory
//! let result = ctx.eval("1 + 2").unwrap();
//! assert_eq!(result.to_i32(), Some(3));
//! ```

#![allow(dead_code)] // During development

// Core modules
pub mod value;
pub mod context;

// Garbage collector
pub mod gc;

// Virtual machine
pub mod vm;

// Parser and compiler
pub mod parser;

// Built-in objects
pub mod builtins;

// Runtime support
pub mod runtime;

// Utilities
pub mod util;

// Re-export main types
pub use context::Context;
pub use value::Value;
