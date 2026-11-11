//! Native Secret Service storage; no plaintext fallback and no automatic unlock prompt.
use dgl_core::{Error, Result, model::Credential};
use secret_service::{EncryptionType, blocking::SecretService};
use std::collections::HashMap;
const SERVICE: &str = "kim.jujin.dglc.rust-candidate.v1";
fn error(e: secret_service::Error) -> Error {
    match e {
        secret_service::Error::Locked => Error::StorageLocked,
        secret_service::Error::Unavailable | secret_service::Error::NoResult => {
            Error::StorageUnavailable
        }
        _ => Error::Storage,
    }
}
fn service() -> Result<SecretService<'static>> {
    SecretService::connect(EncryptionType::Dh).map_err(error)
}
fn attributes(reference: &str) -> HashMap<&str, &str> {
    HashMap::from([("service", SERVICE), ("account", reference)])
}
pub fn read(reference: &str) -> Result<Credential> {
    read_optional(reference)?.ok_or(Error::Storage)
}
pub fn read_optional(reference: &str) -> Result<Option<Credential>> {
    let service = service()?;
    let found = service.search_items(attributes(reference)).map_err(error)?;
    if !found.locked.is_empty() {
        return Err(Error::StorageLocked);
    }
    match found.unlocked.first() {
        Some(item) => {
            let bytes = zeroize::Zeroizing::new(item.get_secret().map_err(error)?);
            serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(|_| Error::Storage)
        }
        None => Ok(None),
    }
}
pub fn write(reference: &str, value: &Credential) -> Result<()> {
    let service = service()?;
    let collection = service.get_default_collection().map_err(error)?;
    if collection.is_locked().map_err(error)? {
        return Err(Error::StorageLocked);
    }
    let bytes = zeroize::Zeroizing::new(serde_json::to_vec(value).map_err(|_| Error::Storage)?);
    collection
        .create_item(
            "DGL credential",
            attributes(reference),
            &bytes,
            true,
            "application/json",
        )
        .map_err(error)?;
    Ok(())
}
pub fn delete(reference: &str) -> Result<()> {
    let service = service()?;
    let found = service.search_items(attributes(reference)).map_err(error)?;
    if !found.locked.is_empty() {
        return Err(Error::StorageLocked);
    }
    for item in found.unlocked {
        item.delete().map_err(error)?;
    }
    Ok(())
}
