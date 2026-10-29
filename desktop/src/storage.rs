#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "macos")]
pub use macos::{delete, read, read_optional, write};
#[cfg(target_os = "windows")]
pub use windows::{delete, read, read_optional, write};

pub fn trust_ref(endpoint: &str) -> String {
    format!("host-key:{endpoint}")
}

/// Why a host's settings fingerprint cannot be used for a connection.
#[derive(Debug, PartialEq, Eq)]
pub enum TrustError {
    /// The settings fingerprint has no matching protected copy.
    Mismatch,
    Storage(dgl_core::Error),
}
impl From<dgl_core::Error> for TrustError {
    fn from(error: dgl_core::Error) -> Self {
        Self::Storage(error)
    }
}

/// A settings fingerprint is trusted only while its protected copy matches. A missing
/// or different copy blocks the connection; it never turns into a first-connection prompt.
pub fn verified_trust(
    expected: &str,
    saved: dgl_core::Result<Option<dgl_core::model::Credential>>,
) -> Result<String, TrustError> {
    match saved? {
        Some(saved) if saved.password == expected => Ok(expected.to_owned()),
        _ => Err(TrustError::Mismatch),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dgl_core::model::Credential;

    fn saved(fingerprint: &str) -> dgl_core::Result<Option<Credential>> {
        Ok(Some(Credential {
            password: fingerprint.into(),
            private_key: String::new(),
            passphrase: String::new(),
        }))
    }

    #[test]
    fn trust_requires_a_matching_protected_copy() {
        assert_eq!(
            verified_trust("SHA256:a", saved("SHA256:a")),
            Ok("SHA256:a".into())
        );
        assert_eq!(
            verified_trust("SHA256:a", saved("SHA256:b")),
            Err(TrustError::Mismatch)
        );
        assert_eq!(
            verified_trust("SHA256:a", Ok(None)),
            Err(TrustError::Mismatch)
        );
        assert_eq!(
            verified_trust("SHA256:a", Err(dgl_core::Error::Storage)),
            Err(TrustError::Storage(dgl_core::Error::Storage))
        );
    }
}
