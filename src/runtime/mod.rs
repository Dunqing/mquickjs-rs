//! Runtime support
//!
//! This module contains the core runtime types for JavaScript execution:
//! - Object representation (JSObject, properties)
//! - String handling (JSString, interning)
//! - Array handling (JSArray)
//! - Function types (closures, C functions, bytecode)
//! - Property operations
//! - Function call mechanics

pub mod array;
pub mod call;
pub mod function;
pub mod object;
pub mod property;
pub mod string;

pub use array::{JSArray, MAX_ARRAY_LENGTH};
pub use function::{CFunction, Closure, FunctionBytecode, FunctionKind, VarRef, CFunctionPtr, MAX_ARGS, CaptureInfo};
pub use object::{
    ArrayBufferData, ArrayData, CFunctionData, ClassId, ClosureData, ErrorData, JSObject,
    ObjectHeader, Property, PropertyType, RegExpData, TypedArrayData, UserData,
};
pub use property::PropertyTable;
pub use string::{JSString, StringTable};
