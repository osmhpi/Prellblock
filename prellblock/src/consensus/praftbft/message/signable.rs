use super::{ConsensusMessage, ConsensusResponse, Metadata};
use crate::consensus::SignatureList;

use pinxit::Signable;
use serde::Serialize;

#[derive(Serialize)]
pub enum SignableData<'a> {
    ConsensusMessage(&'a ConsensusMessage),
    ConsensusResponse(&'a ConsensusResponse),
    AppendMessage {
        metadata: &'a Metadata,
        ackprepare_signatures: &'a SignatureList,
    },
}

impl<'a> Signable for SignableData<'a> {
    type SignableData = Vec<u8>;
    type Error = postcard::Error;
    fn signable_data(&self) -> Result<Self::SignableData, Self::Error> {
        postcard::to_stdvec(self)
    }
}

impl Signable for ConsensusMessage {
    type SignableData = Vec<u8>;
    type Error = postcard::Error;
    fn signable_data(&self) -> Result<Self::SignableData, Self::Error> {
        match self {
            // Skip `data` field of append message. (It is signed via the `block_hash`)
            Self::Append(message) => SignableData::AppendMessage {
                metadata: &message.metadata,
                ackprepare_signatures: &message.ackprepare_signatures,
            },
            _ => SignableData::ConsensusMessage(self),
        }
        .signable_data()
    }
}

impl Signable for ConsensusResponse {
    type SignableData = Vec<u8>;
    type Error = postcard::Error;
    fn signable_data(&self) -> Result<Self::SignableData, Self::Error> {
        SignableData::ConsensusResponse(self).signable_data()
    }
}
