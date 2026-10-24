use dgl_core::{Error, Result, model::Credential};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    path::PathBuf,
    ptr,
};
use windows_sys::Win32::{
    Foundation::LocalFree,
    Security::Cryptography::{
        CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
    },
};
use zeroize::{Zeroize, Zeroizing};

const SERVICE: &str = "kim.jujin.rovessh.rust-candidate.v1";
const MAX_BYTES: usize = 2 * 1024 * 1024;

fn store() -> Result<Store> {
    Ok(Store(crate::platform::data_dir()?.join("credentials")))
}
pub fn read(reference: &str) -> Result<Credential> {
    read_optional(reference)?.ok_or(Error::Storage)
}
pub fn read_optional(reference: &str) -> Result<Option<Credential>> {
    store()?.read(reference)
}
pub fn write(reference: &str, value: &Credential) -> Result<()> {
    store()?.write(reference, value)
}
pub fn delete(reference: &str) -> Result<()> {
    store()?.delete(reference)
}

struct Store(PathBuf);
impl Store {
    fn path(&self, reference: &str) -> Result<PathBuf> {
        if reference.is_empty() || reference.len() > 1024 {
            return Err(Error::InvalidArgument);
        }
        // References include host:port strings. Never interpret them as paths.
        Ok(self
            .0
            .join(format!("{:x}.sealed", Sha256::digest(reference.as_bytes()))))
    }
    fn read(&self, reference: &str) -> Result<Option<Credential>> {
        let file = match std::fs::File::open(self.path(reference)?) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(Error::Storage),
        };
        let mut sealed = Vec::new();
        file.take(MAX_BYTES as u64 + 1)
            .read_to_end(&mut sealed)
            .map_err(|_| Error::Storage)?;
        let bytes = crypt(&sealed, reference, false)?;
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|_| Error::Storage)
    }
    fn write(&self, reference: &str, value: &Credential) -> Result<()> {
        let path = self.path(reference)?;
        let bytes = Zeroizing::new(serde_json::to_vec(value).map_err(|_| Error::Storage)?);
        let sealed = crypt(&bytes, reference, true)?;
        std::fs::create_dir_all(&self.0).map_err(|_| Error::Storage)?;
        // Only ciphertext is written, including temporary files. Persist replaces
        // an existing file without a delete-first interval on Windows.
        let mut temp = tempfile::NamedTempFile::new_in(&self.0).map_err(|_| Error::Storage)?;
        temp.write_all(&sealed)
            .and_then(|_| temp.as_file().sync_all())
            .map_err(|_| Error::Storage)?;
        temp.persist(path).map_err(|_| Error::Storage)?;
        Ok(())
    }
    fn delete(&self, reference: &str) -> Result<()> {
        match std::fs::remove_file(self.path(reference)?) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(Error::Storage),
        }
    }
}

// DPAPI authenticates the ciphertext and binds it to the current Windows user.
// Reference-specific entropy also rejects a blob copied over a different entry.
fn crypt(bytes: &[u8], reference: &str, protect: bool) -> Result<Zeroizing<Vec<u8>>> {
    if bytes.is_empty() || bytes.len() > MAX_BYTES {
        return Err(Error::Storage);
    }
    let input = CRYPT_INTEGER_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_ptr().cast_mut(),
    };
    let entropy_bytes = format!("{SERVICE}:{reference}");
    let entropy = CRYPT_INTEGER_BLOB {
        cbData: entropy_bytes.len() as u32,
        pbData: entropy_bytes.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: ptr::null_mut(),
    };
    // SAFETY: input/entropy remain alive for this synchronous call. DPAPI does
    // not mutate their contents. No machine scope, prompt, or plaintext fallback.
    let ok = unsafe {
        if protect {
            CryptProtectData(
                &input,
                ptr::null(),
                &entropy,
                ptr::null(),
                ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptUnprotectData(
                &input,
                ptr::null_mut(),
                &entropy,
                ptr::null(),
                ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        }
    };
    if ok == 0 {
        return Err(Error::Storage);
    }
    if output.pbData.is_null() {
        return Err(Error::Storage);
    }
    // SAFETY: successful DPAPI calls allocate cbData bytes using LocalAlloc.
    // Copy into zeroizing storage and erase the native buffer before LocalFree.
    unsafe {
        let buffer = std::slice::from_raw_parts_mut(output.pbData, output.cbData as usize);
        let result = if buffer.len() <= MAX_BYTES {
            Ok(Zeroizing::new(buffer.to_vec()))
        } else {
            Err(Error::Storage)
        };
        buffer.zeroize();
        LocalFree(output.pbData.cast());
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dpapi_roundtrip_overwrite_delete_and_no_plaintext() {
        let directory = tempfile::tempdir().unwrap();
        let store = Store(directory.path().join("credentials"));
        let reference = "host-key:[::1]:22";
        assert!(store.read(reference).unwrap().is_none());
        let mut credential = Credential {
            password: "synthetic-windows-secret".into(),
            private_key: String::new(),
            passphrase: String::new(),
        };
        store.write(reference, &credential).unwrap();
        assert_eq!(
            store.read(reference).unwrap().unwrap().password,
            credential.password
        );
        let bytes = std::fs::read(store.path(reference).unwrap()).unwrap();
        assert!(
            !bytes
                .windows(credential.password.len())
                .any(|w| w == credential.password.as_bytes())
        );
        credential.password = "replacement-secret".into();
        store.write(reference, &credential).unwrap();
        assert_eq!(
            store.read(reference).unwrap().unwrap().password,
            credential.password
        );
        store.delete(reference).unwrap();
        store.delete(reference).unwrap();
        assert!(store.read(reference).unwrap().is_none());
    }

    #[test]
    fn dpapi_rejects_tamper_swapped_entries_and_storage_failure() {
        let directory = tempfile::tempdir().unwrap();
        let store = Store(directory.path().join("credentials"));
        let value = Credential {
            password: "synthetic-test-only".into(),
            private_key: String::new(),
            passphrase: String::new(),
        };
        store.write("first", &value).unwrap();
        std::fs::copy(store.path("first").unwrap(), store.path("second").unwrap()).unwrap();
        assert!(store.read("second").is_err());
        let mut bytes = std::fs::read(store.path("first").unwrap()).unwrap();
        *bytes.last_mut().unwrap() ^= 1;
        std::fs::write(store.path("first").unwrap(), bytes).unwrap();
        assert!(store.read("first").is_err());
        assert!(store.path("").is_err());
        assert_eq!(store.path("../outside").unwrap().parent().unwrap(), store.0);
        let blocked = directory.path().join("not-a-directory");
        std::fs::write(&blocked, b"fixture").unwrap();
        assert!(Store(blocked).write("first", &value).is_err());
    }
}
