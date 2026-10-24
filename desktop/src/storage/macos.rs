use dgl_core::{Error, Result, model::Credential};
const SERVICE: &str = "kim.jujin.rovessh.rust-candidate.v1";
pub fn read(reference: &str) -> Result<Credential> {
    read_optional(reference)?.ok_or(Error::Storage)
}
pub fn read_optional(reference: &str) -> Result<Option<Credential>> {
    let bytes = match security_framework::passwords::get_generic_password(SERVICE, reference) {
        Ok(bytes) => bytes,
        Err(e) if e.code() == -25300 => return Ok(None),
        Err(_) => return Err(Error::Storage),
    };
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|_| Error::Storage)
}
pub fn write(reference: &str, value: &Credential) -> Result<()> {
    let bytes = serde_json::to_vec(value).map_err(|_| Error::Storage)?;
    security_framework::passwords::set_generic_password(SERVICE, reference, &bytes)
        .map_err(|_| Error::Storage)
}
pub fn delete(reference: &str) -> Result<()> {
    match security_framework::passwords::delete_generic_password(SERVICE, reference) {
        Ok(()) => Ok(()),
        Err(e) if e.code() == -25300 => Ok(()),
        Err(_) => Err(Error::Storage),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires an unlocked interactive macOS Keychain"]
    fn keychain_roundtrip_and_delete() {
        let reference = format!("fixture-{}", std::process::id());
        let credential = Credential {
            password: "synthetic-test-only".into(),
            private_key: String::new(),
            passphrase: String::new(),
        };
        let bytes = serde_json::to_vec(&credential).unwrap();
        security_framework::passwords::set_generic_password(SERVICE, &reference, &bytes)
            .unwrap_or_else(|error| panic!("Keychain OSStatus {}", error.code()));
        assert_eq!(read(&reference).unwrap().password, credential.password);
        delete(&reference).unwrap();
        assert!(read(&reference).is_err());
    }
}
