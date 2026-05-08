use crate::errors::Error;
use crate::internal_prelude::*;
use crate::state::gen_state_id;
use core::str::FromStr;
use core::time::Duration;
use ethereum_light_client_types::client_state::ClientState as EthClientState;
use ethereum_light_client_types::membership::{verify_membership, verify_non_membership};
use light_client::commitments::{gen_state_id_from_any, EmittedState, MisbehaviourProxyMessage, PrevState, TrustingPeriodContext, UpdateStateProxyMessage, ValidationContext, VerifyMembershipProxyMessage};
use light_client::ibc::IBCContext;
use light_client::types::proto::google::protobuf::Any as IBCAny;
use light_client::types::{Any, ClientId, Height, Time};
use light_client::{
    CreateClientResult, HostClientReader, LightClient, MisbehaviourData, UpdateStateData,
    VerifyMembershipResult, VerifyNonMembershipResult,
};
use tiny_keccak::{Hasher, Keccak};
use crate::client_state::{ClientState, ETHEREUM_CLIENT_REVISION_NUMBER};
use crate::consensus_state::ConsensusState;
use crate::header::{ClientMessage, Header};
use crate::misbehaviour::Misbehaviour;
use ethereum_light_client_types::errors::Error as EthError;

pub struct EthereumLightClient<const SYNC_COMMITTEE_SIZE: usize>;

pub(crate) const ETHEREUM_CLIENT_TYPE: &str = "ethereum";

impl<const SYNC_COMMITTEE_SIZE: usize> LightClient for EthereumLightClient<SYNC_COMMITTEE_SIZE> {
    fn client_type(&self) -> String {
        ETHEREUM_CLIENT_TYPE.into()
    }

    fn latest_height(
        &self,
        ctx: &dyn HostClientReader,
        client_id: &ClientId,
    ) -> Result<Height, light_client::Error> {
        let any_client_state = ctx.client_state(client_id)?;
        let client_state = ClientState::<SYNC_COMMITTEE_SIZE>::try_from(any_client_state)?;
        Ok(client_state.latest_height())
    }

    fn create_client(
        &self,
        _: &dyn HostClientReader,
        any_client_state: Any,
        any_consensus_state: Any,
    ) -> Result<CreateClientResult, light_client::Error> {
        let client_state = ClientState::<SYNC_COMMITTEE_SIZE>::try_from(any_client_state.clone())?;
        let consensus_state = ConsensusState::try_from(any_consensus_state)?;

        client_state.validate()?;
        if client_state.is_frozen() {
            return Err(Error::CannotInitializeFrozenClient.into());
        }

        let height = client_state.latest_height();
        let timestamp = consensus_state.timestamp;
        let state_id = gen_state_id(client_state, consensus_state)?;
        Ok(CreateClientResult {
            height,
            message: UpdateStateProxyMessage {
                prev_height: None,
                prev_state_id: None,
                post_height: height,
                post_state_id: state_id,
                emitted_states: vec![EmittedState(height, any_client_state.into())],
                timestamp,
                context: ValidationContext::Empty,
            }
            .into(),
            prove: false,
        })
    }

    fn update_client(
        &self,
        ctx: &dyn HostClientReader,
        client_id: ClientId,
        any_message: Any,
    ) -> Result<light_client::UpdateClientResult, light_client::Error> {
        let message = ClientMessage::<SYNC_COMMITTEE_SIZE>::try_from(IBCAny::from(any_message.clone()))?;
        match message {
            ClientMessage::Header(header) => Ok(self
                .update_state(ctx, client_id, header)?
                .into()),
            ClientMessage::Misbehaviour(misbehaviour) => Ok(self
                .submit_misbehaviour(ctx, client_id, any_message, misbehaviour)?
                .into()),
        }
    }

    fn verify_membership(
        &self,
        ctx: &dyn HostClientReader,
        client_id: ClientId,
        prefix: Vec<u8>,
        path: String,
        value: Vec<u8>,
        proof_height: Height,
        proof: Vec<u8>,
    ) -> Result<VerifyMembershipResult, light_client::Error> {
        let any_client_state = ctx.client_state(&client_id)?;
        let any_consensus_state = ctx.consensus_state(&client_id, &proof_height)?;
        let client_state = ClientState::<SYNC_COMMITTEE_SIZE>::try_from(any_client_state.clone())?;
        if client_state.is_frozen() {
            return Err(Error::ClientFrozen(client_id).into());
        }
        let consensus_state = ConsensusState::try_from(any_consensus_state)?;
        let value = verify_membership(&client_state, &consensus_state, client_id, path.clone(), value, proof_height, proof, &client_state.execution_verifier)
            .map_err(Error::TypeError)?;
        Ok(VerifyMembershipResult {
            message: VerifyMembershipProxyMessage::new(
                prefix.to_vec(),
                path,
                Some(value),
                proof_height,
                gen_state_id(client_state, consensus_state)?,
            ),
        })
    }

    fn verify_non_membership(
        &self,
        ctx: &dyn HostClientReader,
        client_id: ClientId,
        prefix: Vec<u8>,
        path: String,
        proof_height: Height,
        proof: Vec<u8>,
    ) -> Result<VerifyNonMembershipResult, light_client::Error> {
        let any_client_state = ctx.client_state(&client_id)?;
        let any_consensus_state = ctx.consensus_state(&client_id, &proof_height)?;
        let client_state = ClientState::<SYNC_COMMITTEE_SIZE>::try_from(any_client_state.clone())?;
        if client_state.is_frozen() {
            return Err(Error::ClientFrozen(client_id).into());
        }
        let consensus_state = ConsensusState::try_from(any_consensus_state)?;
        verify_non_membership(&client_state, &consensus_state, client_id, path.clone(), proof_height, proof, &client_state.execution_verifier)
            .map_err(Error::TypeError)?;
        Ok(VerifyNonMembershipResult {
            message: VerifyMembershipProxyMessage::new(
                prefix.to_vec(),
                path,
                None,
                proof_height,
                gen_state_id(client_state, consensus_state)?,
            ),
        })
    }
}

impl<const SYNC_COMMITTEE_SIZE: usize> EthereumLightClient<SYNC_COMMITTEE_SIZE> {
    fn update_state(
        &self,
        ctx: &dyn HostClientReader,
        client_id: ClientId,
        header: Header<SYNC_COMMITTEE_SIZE>,
    ) -> Result<UpdateStateData, light_client::Error> {
        let height = Height::new(ETHEREUM_CLIENT_REVISION_NUMBER, header.execution_update.block_number.0);
        let trusted_height = header.trusted_sync_committee.height;

        let any_client_state = ctx.client_state(&client_id)?;
        let any_consensus_state = ctx.consensus_state(&client_id, &trusted_height)?;

        //Ensure client is not frozen
        let client_state = ClientState::<SYNC_COMMITTEE_SIZE>::try_from(any_client_state)?;
        if client_state.is_frozen() {
            return Err(Error::ClientFrozen(client_id).into());
        }

        // Create new state and ensure header is valid
        let consensus_state = ConsensusState::try_from(any_consensus_state)?;

        let (new_client_state, new_consensus_state) = client_state
            .check_header_and_update_state(
                ctx.host_timestamp(),
                &consensus_state,
                header,
            )?;

        let header_timestamp = new_consensus_state.timestamp;
        let prev_state_id = gen_state_id(client_state.clone(), consensus_state.clone())?;
        let post_state_id = gen_state_id(new_client_state.clone(), new_consensus_state.clone())?;
        Ok(UpdateStateData {
            new_any_client_state: new_client_state.try_into()?,
            new_any_consensus_state: new_consensus_state.try_into()?,
            height,
            message: UpdateStateProxyMessage {
                prev_height: Some(trusted_height.into()),
                prev_state_id: Some(prev_state_id),
                post_height: height,
                post_state_id,
                emitted_states: Default::default(),
                timestamp: header_timestamp,
                context: ValidationContext::TrustingPeriod(TrustingPeriodContext::new(
                    client_state.trusting_period,
                    client_state.max_clock_drift,
                    header_timestamp,
                    consensus_state.timestamp
                )),
            },
            prove: true,
        })
    }

    fn submit_misbehaviour(
        &self,
        ctx: &dyn HostClientReader,
        client_id: ClientId,
        any_message: Any,
        misbehaviour: Misbehaviour<SYNC_COMMITTEE_SIZE>,
    ) -> Result<MisbehaviourData, light_client::Error> {
        let trusted_height = misbehaviour.trusted_sync_committee.height;
        let any_client_state = ctx.client_state(&client_id)?;
        let any_consensus_state = ctx.consensus_state(&client_id, &trusted_height)?;
        //Ensure client is not frozen
        let client_state = ClientState::<SYNC_COMMITTEE_SIZE>::try_from(any_client_state)?;
        if client_state.is_frozen() {
            return Err(Error::ClientFrozen(client_id).into());
        }

        // Create new state and ensure header is valid
        let consensus_state = ConsensusState::try_from(any_consensus_state)?;

        let new_client_state = client_state
            .check_misbehaviour_and_update_state(
                ctx.host_timestamp(),
                &client_id,
                &consensus_state,
                misbehaviour,
            )?;

        Ok(MisbehaviourData {
            new_any_client_state: new_client_state.try_into()?,
            message: MisbehaviourProxyMessage {
                prev_states: self.make_prev_states(
                    ctx,
                    &client_id,
                    &client_state,
                    vec![trusted_height.into()],
                )?,
                // For misbehaviour, it is acceptable if the header's timestamp points to the future.
                context: ValidationContext::TrustingPeriod(TrustingPeriodContext::new(
                    client_state.trusting_period,
                    Duration::ZERO,
                    Time::unix_epoch(),
                    consensus_state.timestamp,
                )),
                client_message: any_message
            },
        })
    }

    fn make_prev_states(
        &self,
        ctx: &dyn HostClientReader,
        client_id: &ClientId,
        client_state: &ClientState<SYNC_COMMITTEE_SIZE>,
        heights: Vec<Height>,
    ) -> Result<Vec<PrevState>, Error> {
        let mut prev_states = Vec::new();
        for height in heights {
            let consensus_state: ConsensusState = ctx
                .consensus_state(client_id, &height)?
                .try_into()?;
            prev_states.push(PrevState {
                height,
                state_id: gen_state_id(client_state.clone(), consensus_state)?,
            });
        }
        Ok(prev_states)
    }
}
