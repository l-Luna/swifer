#![feature(layout_for_ptr)]
#![feature(set_ptr_value)]

//! # Swifer!
//!
//! Swifer is a garbage collection library, providing both garbage collectors for use by
//! language runtimes, and tools for the GC implementations themselves, while providing
//! a uniform interface.

pub mod heap;
pub mod gc;

#[cfg(test)]
mod tests;
