// src/backend/storage/storable.rs
use ic_stable_structures::{storable::Bound, Storable};
use serde::{de::DeserializeOwned, Serialize};
use std::borrow::Cow;
use candid::Deserialize;

/// Helper struct to wrap any type T that implements Serialize and DeserializeOwned
/// to make it Storable using CBOR encoding.
#[derive(Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct Cbor<T>(pub T)
where
    T: Serialize + DeserializeOwned;

impl<T> Storable for Cbor<T>
where
    T: Serialize + DeserializeOwned,
{
    fn to_bytes(&self) -> Cow<[u8]> {
        let mut writer = vec![];
        ciborium::ser::into_writer(&self.0, &mut writer)
            .expect("Failed to serialize value to CBOR for stable storage");
        Cow::Owned(writer)
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        let value: T = ciborium::de::from_reader(bytes.as_ref())
            .expect("Failed to deserialize value from CBOR from stable storage");
        Cbor(value)
    }

    // Use unbounded storage for simplicity initially.
    // If specific bounds are needed later, define them per type.
    const BOUND: Bound = Bound::Unbounded;
}

// Define a simple Storable key type using String
// Note: For performance with large numbers of keys, consider fixed-size keys
// or specialized key types if possible.
pub type StorableString = Cbor<String>;

// Example: Making VaultConfig storable using the Cbor wrapper
// pub type StorableVaultConfig = Cbor<crate::models::VaultConfig>;

// You can define specific storable types for each model here or directly use Cbor<ModelType>
// when defining the StableBTreeMap. 