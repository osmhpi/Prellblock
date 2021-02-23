use super::{message::Request, ConsensusMessage, Error, Queue};
use crate::{
    block_storage::BlockStorage,
    consensus::{LeaderTerm, SignatureList, TransactionApplier},
    peer::{message as peer_message, Sender},
    transaction_checker::TransactionChecker,
    world_state::WorldStateService,
};
use balise::Address;
use futures::{stream::FuturesUnordered, StreamExt};
use newtype_enum::Enum;
use pinxit::{Identity, PeerId, Signable, Signed, Verified};
use prellblock_client_api::Transaction;
use tokio::sync::{Mutex, Notify};

#[derive(Debug)]
pub struct Core {
    pub(super) identity: Identity,
    pub(super) block_storage: BlockStorage,
    pub(super) world_state: WorldStateService,
    pub(super) transaction_applier: TransactionApplier,
    pub(super) transaction_checker: TransactionChecker,
    pub(super) queue: Mutex<Queue<Signed<Transaction>>>,
    pub(super) notify_censorship_checker: Notify,
    pub(super) notify_leader: Notify,
}

impl Core {
    pub fn new(
        identity: Identity,
        block_storage: BlockStorage,
        world_state: WorldStateService,
        transaction_applier: TransactionApplier,
    ) -> Self {
        Self {
            identity,
            block_storage,
            world_state: world_state.clone(),
            transaction_applier,
            transaction_checker: TransactionChecker::new(world_state),
            queue: Mutex::default(),
            notify_censorship_checker: Notify::new(),
            notify_leader: Notify::new(),
        }
    }

    pub fn leader(&self, leader_term: LeaderTerm) -> PeerId {
        let peers = self.world_state.get().peers;
        let index = u64::from(leader_term) % (peers.len() as u64);
        #[allow(clippy::cast_possible_truncation)]
        peers[index as usize].0.clone()
    }

    pub fn verify_rpu_majority_signatures<E>(
        &self,
        message: impl newtype_enum::Variant<E>,
        signatures: &SignatureList,
    ) -> Result<(), Error>
    where
        E: newtype_enum::Enum + Signable,
    {
        if !signatures.is_unique() {
            return Err(Error::DuplicateSignatures);
        }

        if !self.supermajority_reached(signatures.len()) {
            return Err(Error::NotEnoughSignatures);
        }

        let message = Enum::from_variant(message);
        for (peer_id, signature) in signatures {
            // All signatures in here must be valid.
            // The leader would filter out any wrong signatures.
            peer_id.verify(&message, signature)?;

            // Also check whether the signer is a known RPU
            self.transaction_checker
                .account_checker(peer_id.clone())?
                .verify_is_rpu()?;
        }

        Ok(())
    }

    #[allow(clippy::future_not_send)]
    pub async fn send_message<M>(
        &self,
        peer_address: Address,
        message: M,
    ) -> Result<Verified<M::Response>, Error>
    where
        M: Request,
    {
        let signed_message = self.sign_message(message)?;
        send_signed_message::<M>(peer_address, signed_message).await
    }

    #[allow(clippy::future_not_send)]
    pub async fn broadcast_until_majority<M, F>(
        &self,
        message: M,
        verify_response: F,
    ) -> Result<SignatureList, Error>
    where
        M: Request,
        F: Fn(&M::Response) -> Result<(), Error> + Clone + Send + Sync + 'static,
    {
        let signed_message = self.sign_message(message)?;

        let mut futures = FuturesUnordered::new();

        let peers = self.world_state.get().peers;
        let peers_count = peers.len();
        for (peer_id, peer_address) in peers {
            let address = peer_address.clone();
            let signed_message = signed_message.clone();
            let verify_response = verify_response.clone();

            futures.push(tokio::spawn(async move {
                let send_message_and_verify_response = async {
                    let verified_response =
                        send_signed_message::<M>(address, signed_message).await?;
                    let signer = verified_response.signer().clone();
                    if signer == peer_id {
                        verify_response(&*verified_response)?;
                        Ok((signer, verified_response.signature().clone()))
                    } else {
                        Err(Error::InvalidPeer(signer))
                    }
                };

                match send_message_and_verify_response.await {
                    Ok(response) => Some(response),
                    Err(err) => {
                        log::warn!("Consensus error from {}: {}", peer_address, err);
                        None
                    }
                }
            }));
        }

        let mut responses = SignatureList::default();

        while let Some(result) = futures.next().await {
            match result {
                Ok(Some(response)) => {
                    responses.push(response);
                }
                Ok(None) => {}
                Err(err) => log::warn!("Failed to join task: {}", err),
            }
            if supermajority_reached(responses.len(), peers_count) {
                return Ok(responses);
            }
        }

        // All sender tasks have died **before reaching supermajority**.
        Err(Error::CouldNotGetSupermajority)
    }

    fn sign_message<M>(&self, message: M) -> Result<peer_message::Consensus, Error>
    where
        M: Request,
    {
        let message = ConsensusMessage::from_variant(message);
        let message = message.sign(&self.identity)?;
        Ok(peer_message::Consensus(message))
    }

    /// Check whether a number represents a supermajority (>2/3) compared
    /// to the total number of peers in the consenus.
    pub fn supermajority_reached(&self, response_len: usize) -> bool {
        supermajority_reached(response_len, self.world_state.get().peers.len())
    }
}

async fn send_signed_message<M>(
    peer_address: Address,
    signed_message: peer_message::Consensus,
) -> Result<Verified<M::Response>, Error>
where
    M: Request,
{
    let mut sender = Sender::new(peer_address);
    let response = sender.send_request(signed_message).await?;
    let response = response.verify()?;
    response.try_map(|response| response.into_variant().ok_or(Error::UnexpectedResponse))
}

/// Check whether a number represents a supermajority (>2/3) compared
/// to the total number of peers (`peer_count`) in the consenus.
pub fn supermajority_reached(response_len: usize, peer_count: usize) -> bool {
    if peer_count < 4 {
        panic!("Cannot find consensus for less than four peers.");
    }
    let supermajority = peer_count * 2 / 3 + 1;
    response_len >= supermajority
}
