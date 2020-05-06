use pinxit::{PeerId, Signature};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, iter::FromIterator};

type SignatureListItem = (PeerId, Signature);
type SignatureListVec = Vec<SignatureListItem>;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SignatureList(SignatureListVec);

impl SignatureList {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn push(&mut self, item: SignatureListItem) {
        self.0.push(item);
    }

    pub fn is_unique(&self) -> bool {
        let mut set = HashSet::new();
        self.0.iter().all(|(peer_id, _)| set.insert(peer_id))
    }
}

impl<'a> IntoIterator for &'a SignatureList {
    type Item = <&'a SignatureListVec as IntoIterator>::Item;
    type IntoIter = <&'a SignatureListVec as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl FromIterator<SignatureListItem> for SignatureList {
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = SignatureListItem>,
    {
        Self(Vec::from_iter(iter))
    }
}
