//! Runtime support
//!
//! This module contains the core runtime types for JavaScript execution:
//! - Object representation (JSObject, properties)
//! - String handling (JSString, interning)
//! - Property operations
//! - Function call mechanics

pub mod call;
pub mod object;
pub mod property;
pub mod string;

pub use object::{
    ArrayBufferData, ArrayData, CFunctionData, ClassId, ClosureData, ErrorData, JSObject,
    ObjectHeader, Property, PropertyType, RegExpData, TypedArrayData, UserData,
};
pub use property::PropertyTable;
pub use string::{JSString, StringTable};
