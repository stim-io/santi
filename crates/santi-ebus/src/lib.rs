//! Event-bus related adapter implementations.
//!
//! Canonical implementations live under `adapter/{local,redis}`.
//! Current runtime wiring uses the local subscriber-set adapter on the main path.

pub mod adapter;
