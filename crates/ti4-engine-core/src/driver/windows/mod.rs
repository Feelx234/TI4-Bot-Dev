//! Windows ported onto the driver. The worked example of docs/ENGINE_CORE_DRIVER.md is
//! [`sustain_commander`], which package B1 moved to `rules/combat/` (it is re-exported here so the driver
//! tests and adapters keep their path).

pub use crate::rules::combat::sustain_commander;

pub mod aftermath;
pub mod chain;
pub mod combat;
pub mod enforce_all;
pub mod live_common;
pub mod invasion;
pub mod minister;
pub mod scoring;
pub mod tactical;
