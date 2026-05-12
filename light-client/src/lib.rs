#![cfg_attr(not(test), no_std)]
#![allow(clippy::result_large_err)]
#![allow(clippy::large_enum_variant)]

use client::EthereumLightClient;
use light_client::LightClientRegistry;
extern crate alloc;

pub mod client;
pub mod errors;
pub mod state;

pub mod client_state;
pub mod consensus_state;

pub mod header;
pub mod misbehaviour;

#[cfg(test)]
pub(crate) mod test_utils;

#[allow(unused_imports)]
mod internal_prelude {
    pub use alloc::boxed::Box;
    pub use alloc::format;
    pub use alloc::string::{String, ToString};
    pub use alloc::vec;
    pub use alloc::vec::Vec;
}
use crate::client_state::ETHEREUM_CLIENT_STATE_TYPE_URL;
use internal_prelude::*;

pub fn register_implementations<const SYNC_COMMITTEE_SIZE: usize>(
    registry: &mut dyn LightClientRegistry,
) {
    registry
        .put_light_client(
            ETHEREUM_CLIENT_STATE_TYPE_URL.to_string(),
            Box::new(EthereumLightClient::<SYNC_COMMITTEE_SIZE>),
        )
        .unwrap()
}
