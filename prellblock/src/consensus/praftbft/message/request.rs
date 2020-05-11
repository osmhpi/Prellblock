use super::{consensus_message, consensus_response, ConsensusMessage, ConsensusResponse};
use newtype_enum::Variant;

pub trait Request: Variant<ConsensusMessage> {
    type Response: Variant<ConsensusResponse>;
}

impl Request for consensus_message::Prepare {
    type Response = consensus_response::AckPrepare;
}

impl Request for consensus_message::Append {
    type Response = consensus_response::AckAppend;
}

impl Request for consensus_message::Commit {
    type Response = consensus_response::Ok;
}

impl Request for consensus_message::ViewChange {
    type Response = consensus_response::Ok;
}

impl Request for consensus_message::NewView {
    type Response = consensus_response::Ok;
}

impl Request for consensus_message::SynchronizationRequest {
    type Response = consensus_response::SynchronizationResponse;
}
