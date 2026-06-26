//! Test utilities for Ethereum light client tests.
//!
//! This module provides common helper functions and types for testing,
//! reducing code duplication across test modules.

use ethereum_consensus::config;
use ethereum_consensus::context::ChainContext;
use ethereum_consensus::preset::minimal::PRESET;
use ethereum_consensus::types::{Address, H256, U64};
use ethereum_light_client_types::consensus::ConsensusUpdateInfo;
use ethereum_light_client_verifier::consensus::test_utils::{
    gen_light_client_update_with_params, MockSyncCommitteeManager,
};
use ethereum_light_client_verifier::context::{Fraction, LightClientContext};
use ethereum_light_client_verifier::updates::ExecutionUpdateInfo;
use hex_literal::hex;

use crate::client_state::ClientState;
use crate::consensus_state::ConsensusState;
use ethereum_consensus::beacon::Slot;
use ethereum_light_client_types::consensus::TrustedSyncCommittee;
use light_client::types::{Height, Time};
use std::time::Duration;

/// Sync committee size from minimal preset
pub const SYNC_COMMITTEE_SIZE: usize = PRESET.SYNC_COMMITTEE_SIZE;

/// Type alias for test client state
pub type TestClientState = ClientState<SYNC_COMMITTEE_SIZE>;

/// Type alias for MockSyncCommitteeManager with minimal preset
pub type TestMockSyncCommitteeManager = MockSyncCommitteeManager<SYNC_COMMITTEE_SIZE>;

/// Default genesis time for tests (2020-01-01 00:00:00 UTC)
pub const DEFAULT_GENESIS_TIME: u64 = 1577836800;

/// Creates a LightClientContext with the given parameters.
pub fn create_light_client_context(genesis_time: u64, current_time: u64) -> LightClientContext {
    // Use a non-zero genesis_validators_root for valid client state
    let genesis_validators_root = H256::from_slice(&[1u8; 32]);
    LightClientContext::new_with_config(
        config::minimal::get_config(),
        genesis_validators_root,
        genesis_time.into(),
        Fraction::new(2, 3).unwrap(),
        current_time.into(),
    )
}

/// Creates a simple LightClientContext with default genesis time.
pub fn create_simple_context(now_secs: u64) -> LightClientContext {
    // Use a non-zero genesis_validators_root for valid client state
    let genesis_validators_root = H256::from_slice(&[1u8; 32]);
    LightClientContext::new_with_config(
        config::minimal::get_config(),
        genesis_validators_root,
        Default::default(),
        Fraction::new(2, 3).unwrap(),
        now_secs.into(),
    )
}

/// Converts verifier's ConsensusUpdateInfo to types' ConsensusUpdateInfo.
pub fn to_consensus_update_info<const SYNC_COMMITTEE_SIZE: usize>(
    consensus_update: ethereum_light_client_verifier::updates::ConsensusUpdateInfo<
        SYNC_COMMITTEE_SIZE,
    >,
) -> ConsensusUpdateInfo<SYNC_COMMITTEE_SIZE> {
    ConsensusUpdateInfo {
        attested_header: consensus_update.light_client_update.attested_header,
        next_sync_committee: consensus_update.light_client_update.next_sync_committee,
        finalized_header: consensus_update.light_client_update.finalized_header,
        sync_aggregate: consensus_update.light_client_update.sync_aggregate,
        signature_slot: consensus_update.light_client_update.signature_slot,
        finalized_execution_root: consensus_update.finalized_execution_root,
        finalized_execution_branch: consensus_update.finalized_execution_branch,
    }
}

/// Test fixture containing common test data.
pub struct TestFixture {
    pub scm: TestMockSyncCommitteeManager,
    pub ctx: LightClientContext,
    pub period_1: U64,
    pub base_signature_slot: U64,
    pub base_attested_slot: U64,
    pub base_finalized_epoch: U64,
}

impl TestFixture {
    /// Creates a new test fixture with default parameters.
    pub fn new() -> Self {
        Self::with_genesis_time(DEFAULT_GENESIS_TIME)
    }

    /// Creates a new test fixture with custom genesis time.
    pub fn with_genesis_time(genesis_time: u64) -> Self {
        let current_time = genesis_time + 100_000;
        let scm = MockSyncCommitteeManager::new(1, 4);
        let ctx = create_light_client_context(genesis_time, current_time);

        let period_1 = U64(1) * ctx.slots_per_epoch() * ctx.epochs_per_sync_committee_period();
        let base_signature_slot = period_1 + 11;
        let base_attested_slot = base_signature_slot - 1;
        let base_finalized_epoch = base_attested_slot / ctx.slots_per_epoch();

        Self {
            scm,
            ctx,
            period_1,
            base_signature_slot,
            base_attested_slot,
            base_finalized_epoch,
        }
    }

    /// Creates a new test fixture with simple context (no genesis time offset).
    pub fn simple(now_secs: u64) -> Self {
        let scm = MockSyncCommitteeManager::new(1, 4);
        let ctx = create_simple_context(now_secs);

        let period_1 = U64(1) * ctx.slots_per_epoch() * ctx.epochs_per_sync_committee_period();
        let base_signature_slot = period_1 + 11;
        let base_attested_slot = base_signature_slot - 1;
        let base_finalized_epoch = base_attested_slot / ctx.slots_per_epoch();

        Self {
            scm,
            ctx,
            period_1,
            base_signature_slot,
            base_attested_slot,
            base_finalized_epoch,
        }
    }

    /// Gets the current sync committee (period 1).
    pub fn current_sync_committee(
        &self,
    ) -> &ethereum_light_client_verifier::consensus::test_utils::MockSyncCommittee<
        SYNC_COMMITTEE_SIZE,
    > {
        self.scm.get_committee(1)
    }

    /// Gets the next sync committee (period 2).
    pub fn next_sync_committee(
        &self,
    ) -> &ethereum_light_client_verifier::consensus::test_utils::MockSyncCommittee<
        SYNC_COMMITTEE_SIZE,
    > {
        self.scm.get_committee(2)
    }

    /// Generates a light client update with default parameters.
    pub fn gen_update(
        &self,
        execution_state_root: H256,
        execution_block_number: u64,
    ) -> (
        ethereum_light_client_verifier::updates::ConsensusUpdateInfo<SYNC_COMMITTEE_SIZE>,
        ExecutionUpdateInfo,
    ) {
        gen_light_client_update_with_params::<SYNC_COMMITTEE_SIZE, _>(
            &self.ctx,
            self.base_signature_slot,
            self.base_attested_slot,
            self.base_finalized_epoch,
            execution_state_root,
            execution_block_number.into(),
            self.current_sync_committee(),
            self.next_sync_committee(),
            true,
            PRESET.SYNC_COMMITTEE_SIZE,
        )
    }

    /// Builds a `ConsensusState` whose `current`/`next` sync committee aggregate
    /// pubkeys are the fixture's current/next committees (the common test case).
    pub fn consensus_state(
        &self,
        slot: Slot,
        storage_root: H256,
        timestamp: Time,
    ) -> ConsensusState {
        ConsensusState {
            slot,
            storage_root,
            timestamp,
            current_sync_committee: self
                .current_sync_committee()
                .to_committee()
                .aggregate_pubkey
                .clone(),
            next_sync_committee: self
                .next_sync_committee()
                .to_committee()
                .aggregate_pubkey
                .clone(),
        }
    }

    /// Builds a `TrustedSyncCommittee` backed by the fixture's current committee.
    pub fn trusted_from_current(
        &self,
        height: Height,
        is_next: bool,
    ) -> TrustedSyncCommittee<SYNC_COMMITTEE_SIZE> {
        TrustedSyncCommittee {
            height,
            sync_committee: self.current_sync_committee().to_committee(),
            is_next,
        }
    }

    /// Builds a `TrustedSyncCommittee` backed by the fixture's next committee.
    pub fn trusted_from_next(
        &self,
        height: Height,
        is_next: bool,
    ) -> TrustedSyncCommittee<SYNC_COMMITTEE_SIZE> {
        TrustedSyncCommittee {
            height,
            sync_committee: self.next_sync_committee().to_committee(),
            is_next,
        }
    }
}

impl Default for TestFixture {
    fn default() -> Self {
        Self::new()
    }
}

/// Test account proof data from real Ethereum state.
pub mod account_proof {
    use super::*;

    /// Returns a valid account proof for testing.
    pub fn get_proof() -> Vec<Vec<u8>> {
        vec![
            hex!("f901d180a09199d4ddc4f618c0df40c0e1e09eaf2394cd21d566d841b654f3f268196922d0a0bac36050a7d1931b8d6f027075410a85587c649f2d0b30e8ffe967cb3329314ca03919f1f0815704a954616d26504c9201132454a1c0023252294c1abbe0fab26fa0e72e174077c047357c47cba596110765043277d24c55c787ecb164e33a7f1aa5a0de86ea5531307567132648d5c7956cb6082d6803f3dbc9e16b2dd20b320ca93aa0c2c799b60a0cd6acd42c1015512872e86c186bcf196e85061e76842f3b7cf860a088126df40baa53d4d60c0e2a004b6ee8506f131573c750649380e74662093855a02e0d86c3befd177f574a20ac63804532889077e955320c9361cd10b7cc6f5809a0c326f61dd1e74e037d4db73aede5642260bf92869081753bbace550a73989aeda069d63e492e4c3aa54393df9bc12809c9bfc6482b3feb16f2877d7a3e6857d94780a029087b3ba8c5129e161e2cb956640f4d8e31a35f3f133c19a1044993def98b61a08d65cbe14c995d8fe7c7343e9aa31efc7dd81acb0ee940ee565613d8bbbbaa02a0bb12ddf18cf418b9bb5164d2c0caad9e4a29bdca8f1a0c9ed16dd8095f8792fba0144540d36e30b250d25bd5c34d819538742dc54c2017c4eb1fabb8e45f72759180").to_vec(),
            hex!("f8518080a0b595706019b55ae9c4784db71e12bc68d3c991fc1277327e8d63014d10137f7b8080808080808080a04e41195493413c0bbe1fd524bbac490ed81e002fbf4d3d769e0be3452466de0c8080808080").to_vec(),
            hex!("f869a020fff6b964c3925a3b7475bdd2ad96660593de57a6a55a3ef0c82303af814889b846f8440180a02988bb89d212527a6054fec481672b5cdd01bdf7287129442e82bb7569a412f9a0cf76e7c6fa61cca89fee643691266bb1f2721c2d2eeb3063a5e545560abc2b7a").to_vec(),
        ]
    }

    /// Returns the state root corresponding to the test account proof.
    pub fn get_state_root() -> H256 {
        H256::from_slice(&hex!(
            "6a3c41347943fdeab40fb6f0cff088bc81032c86a22b69c67c83b79b72cbb0b4"
        ))
    }

    /// Returns the IBC address corresponding to the test account proof.
    pub fn get_address() -> Address {
        Address(hex!("12496c9aa0e6754c897ca88c1d53fea9b19b8aff"))
    }

    /// Returns the storage root corresponding to the test account proof.
    pub fn get_storage_root() -> H256 {
        H256::from_slice(&hex!(
            "2988BB89D212527A6054FEC481672B5CDD01BDF7287129442E82BB7569A412F9"
        ))
    }
}

/// Creates a test client state with the given context parameters.
pub fn create_test_client_state_from_ctx(ctx: &LightClientContext) -> TestClientState {
    use ethereum_light_client_verifier::context::ConsensusVerificationContext;

    TestClientState {
        genesis_validators_root: ctx.genesis_validators_root(),
        min_sync_committee_participants: 1u64.into(),
        genesis_time: ctx.genesis_time(),
        fork_parameters: ctx.fork_parameters().clone(),
        seconds_per_slot: PRESET.SECONDS_PER_SLOT,
        slots_per_epoch: PRESET.SLOTS_PER_EPOCH,
        epochs_per_sync_committee_period: PRESET.EPOCHS_PER_SYNC_COMMITTEE_PERIOD,
        ibc_address: account_proof::get_address(),
        ibc_commitments_slot: H256::from_slice(&[2u8; 32]),
        trust_level: Fraction::new(2, 3).unwrap(),
        trusting_period: Duration::from_secs(60 * 60 * 24 * 7), // 1 week
        max_clock_drift: Duration::from_secs(60 * 10),          // 10 minutes
        latest_execution_block_number: 1u64.into(),
        ..Default::default()
    }
}
