//! Bowtie catalog: types, building, and query logic.
//!
//! A "bowtie" is a shared LCC event ID that has at least one producer slot
//! and at least one consumer slot across the discovered nodes on the network.

pub mod catalog;
pub mod types;
