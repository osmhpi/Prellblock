use super::{ConsensusMessage, ConsensusResponse};

use pinxit::Signable;
use serde::Serialize;

#[derive(Serialize)]
pub enum SignableData<'a> {
    ConsensusMessage(&'a ConsensusMessage),
    ConsensusResponse(&'a ConsensusResponse),
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
        SignableData::ConsensusMessage(self).signable_data()
    }
}

impl Signable for ConsensusResponse {
    type SignableData = Vec<u8>;
    type Error = postcard::Error;
    fn signable_data(&self) -> Result<Self::SignableData, Self::Error> {
        SignableData::ConsensusResponse(self).signable_data()
    }
}
