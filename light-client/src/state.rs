use ethereum_light_client_types::client_state::ClientState as _;
use crate::errors::Error;
use light_client::commitments::{gen_state_id_from_any, StateID};
use light_client::types::Any;
use crate::client_state::ClientState;
use crate::consensus_state::ConsensusState;

fn gen_state_id<const SYNC_COMMITTEE_SIZE: usize>(
    client_state: ClientState<SYNC_COMMITTEE_SIZE>,
    consensus_state: ConsensusState,
) -> Result<StateID, Error> {
    let client_state = Any::try_from(client_state.canonicalize())?;
    let consensus_state = Any::try_from(consensus_state)?;
    gen_state_id_from_any(&client_state, &consensus_state)
        .map_err(LightClientError::commitment)
}