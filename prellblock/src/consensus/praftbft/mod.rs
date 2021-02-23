mod censorship_checker;
mod core;
mod error;
mod follower;
mod leader;
mod message;
mod notify;
mod queue;
mod ring_buffer;
mod view_change;

pub use error::Error;
pub use message::{ConsensusMessage, ConsensusResponse};
pub use queue::Queue;
pub use ring_buffer::RingBuffer;

use self::core::Core;
use super::TransactionApplier;
use crate::{block_storage::BlockStorage, world_state::WorldStateService};
use censorship_checker::CensorshipChecker;
use error::ErrorVerify;
use follower::Follower;
use leader::Leader;
use message::Request;
use newtype_enum::Enum;
use notify::NotifyMap;
use pinxit::{Identity, Signable, Signed};
use prellblock_client_api::Transaction;
use std::sync::Arc;
use view_change::ViewChange;

const MAX_TRANSACTIONS_PER_BLOCK: usize = 4000;

type InvalidTransaction = (usize, Signed<Transaction>);

/// See the [paper](https://www.scs.stanford.edu/17au-cs244b/labs/projects/clow_jiang.pdf).
#[derive(Debug)]
#[must_use]
pub struct PRaftBFT {
    core: Arc<Core>,
    follower: Arc<Follower>,
    view_change: Arc<ViewChange>,
}

impl PRaftBFT {
    /// Create new `PRaftBFT` Instance.
    ///
    /// The instance is identified `identity` and in a group with other `peers`.
    /// **Warning:** This starts a new thread for processing transactions in the background.
    pub async fn new(
        identity: Identity,
        block_storage: BlockStorage,
        world_state: WorldStateService,
    ) -> Arc<Self> {
        log::debug!("Started consensus.");

        let transaction_applier =
            TransactionApplier::new(block_storage.clone(), world_state.clone());

        // Setup core
        let core = Arc::new(Core::new(
            identity,
            block_storage,
            world_state,
            transaction_applier,
        ));

        // Setup view_change
        let view_change = Arc::new(ViewChange::new(core.clone()));
        tokio::spawn(view_change.clone().new_view_timeout_checker());

        // Setup follower
        let follower = Arc::new(Follower::new(core.clone(), view_change.clone()));

        // Setup censorship_checker
        let censorship_checker = CensorshipChecker::new(core.clone(), view_change.clone());
        tokio::spawn(censorship_checker.execute());

        // Setup leader
        let leader = Leader::new(core.clone(), follower.clone(), view_change.clone());
        tokio::spawn(leader.execute());

        // Setup consensus
        Arc::new(Self {
            core,
            follower,
            view_change,
        })
    }

    /// Stores incoming `Transaction`s in the Consensus' `queue`.
    pub async fn take_transactions(&self, transactions: Vec<Signed<Transaction>>) {
        let queue_len = {
            let mut queue = self.core.queue.lock().await;
            queue.extend(transactions);
            queue.len()
        };

        if queue_len > MAX_TRANSACTIONS_PER_BLOCK {
            self.core.notify_leader.notify_one();
        }
    }

    /// Process the incoming `ConsensusMessages`.
    pub async fn handle_message(
        self: &Arc<Self>,
        message: Signed<ConsensusMessage>,
    ) -> Result<Signed<ConsensusResponse>, Error> {
        let peer_id = message.signer().clone();

        // Only RPUs are allowed.
        self.core
            .transaction_checker
            .account_checker(peer_id.clone())?
            .verify_is_rpu()?;

        let signature = message.signature().clone();
        let message = message.verify()?;

        macro_rules! dispatch {
            ($(
                $name:ident($message:ident) => $block:expr,
            )*) => {match message.into_inner() {$(
                ConsensusMessage::$name($message) => {
                    get_response_converter(&$message)($block)
                },
            )*}
        };
        }

        let response: ConsensusResponse = dispatch! {
            Prepare(message) => self.follower.handle_prepare_message(peer_id, message).await?,
            Append(message) => self.follower.handle_append_message(peer_id, message).await?,
            Commit(message) => self.follower.handle_commit_message(peer_id, message).await?,
            ViewChange(message) => self.view_change.handle_view_change(peer_id, signature, message.new_leader_term)?,
            NewView(message) => self.follower.handle_new_view_message(peer_id, message).await?,
            SynchronizationRequest(message) => self.follower.handle_synchronization_request(peer_id, message).await?,
        };

        Ok(response.sign(&self.core.identity)?)
    }
}

fn get_response_converter<T>(_: &T) -> fn(T::Response) -> ConsensusResponse
where
    T: Request,
{
    ConsensusResponse::from_variant
}
