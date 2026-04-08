//! Event-bus related adapter implementations.
//!
//! Canonical implementations live under `adapter/{standalone,redis}`.
//! Current runtime wiring uses the standalone subscriber-set adapter on the main path.

pub mod adapter;
