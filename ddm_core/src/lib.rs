//! The `ddm_core` crate provides an API for encoding, decoding and manipulating Delay-Doppler Map
//! data.
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
extern crate std;

#[cfg(not(feature = "std"))]
extern crate alloc;

pub mod coding;
pub mod error;
pub mod l0b;
