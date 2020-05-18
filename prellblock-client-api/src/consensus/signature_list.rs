use pinxit::{PeerId, Signature};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, iter::FromIterator};

type SignatureListItem = (PeerId, Signature);
type SignatureListItemRef<'a> = (&'a PeerId, &'a Signature);
type SignatureListVec = Vec<SignatureListItem>;

/// A list of `PeerId`s and `Signature`s.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SignatureList(SignatureListVec);

impl SignatureList {
    /// Get the current number of signatures in the list.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }
    /// Check whether the list is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Push a `SignatureListItem` to the `SignatureList`.
    pub fn push(&mut self, item: SignatureListItem) {
        self.0.push(item);
    }

    /// Verify that all signatures in the list are from distinct peers.
    #[must_use]
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

impl<'a> FromIterator<SignatureListItemRef<'a>> for SignatureList {
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = SignatureListItemRef<'a>>,
    {
        iter.into_iter()
            .map(|(peer_id, signature)| (peer_id.clone(), signature.clone()))
            .collect()
    }
}
