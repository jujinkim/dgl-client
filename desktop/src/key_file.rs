use std::{
    io::{self, Read},
    path::{Path, PathBuf},
};

pub const MAX_KEY_BYTES: usize = 128 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub enum ImportTarget {
    Choose(Option<PathBuf>),
    Read(PathBuf),
}

pub fn default_directory(home: Option<PathBuf>) -> Option<PathBuf> {
    home.map(|home| {
        let ssh = home.join(".ssh");
        if ssh.is_dir() { ssh } else { home }
    })
}

pub fn target(input: &str, home: Option<PathBuf>) -> ImportTarget {
    if input.trim().is_empty() {
        ImportTarget::Choose(default_directory(home))
    } else {
        // Keep spaces in actual filenames; only whitespace-only input is empty.
        let path = std::path::absolute(input).unwrap_or_else(|_| PathBuf::from(input));
        if path.is_dir() {
            ImportTarget::Choose(Some(path))
        } else {
            ImportTarget::Read(path)
        }
    }
}

fn read_error(error: io::Error) -> &'static str {
    match error.kind() {
        io::ErrorKind::NotFound => "개인키 파일을 찾을 수 없습니다. 경로를 확인하세요.",
        io::ErrorKind::PermissionDenied => "개인키 파일을 읽을 권한이 없습니다.",
        _ => "개인키 파일을 읽지 못했습니다. 파일 상태를 확인하세요.",
    }
}

pub fn read(path: &Path) -> Result<String, &'static str> {
    let file = std::fs::File::open(path).map_err(read_error)?;
    if !file.metadata().map_err(read_error)?.is_file() {
        return Err("일반 개인키 파일을 선택하세요.");
    }
    let mut bytes = Vec::new();
    file.take((MAX_KEY_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(read_error)?;
    if bytes.len() > MAX_KEY_BYTES {
        return Err("개인키 파일은 최대 128 KiB까지 가져올 수 있습니다.");
    }
    if bytes.is_empty() {
        return Err("개인키 파일이 비어 있습니다.");
    }
    String::from_utf8(bytes).map_err(|_| "개인키 파일은 UTF-8 텍스트여야 합니다.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_empty_folder_and_extensionless_unicode_file() {
        let home = tempfile::tempdir().unwrap();
        let root = home.path().to_path_buf();
        assert_eq!(
            target("", Some(root.clone())),
            ImportTarget::Choose(Some(root.clone()))
        );
        let ssh = root.join(".ssh");
        std::fs::create_dir(&ssh).unwrap();
        assert_eq!(
            target("  ", Some(root.clone())),
            ImportTarget::Choose(Some(ssh.clone()))
        );
        assert_eq!(
            target(ssh.to_str().unwrap(), None),
            ImportTarget::Choose(Some(ssh.clone()))
        );
        let key = ssh.join("한글 개인키");
        std::fs::write(&key, "synthetic key").unwrap();
        assert_eq!(
            target(key.to_str().unwrap(), None),
            ImportTarget::Read(key.clone())
        );
        assert_eq!(read(&key).unwrap(), "synthetic key");
        assert_eq!(target("", None), ImportTarget::Choose(None));
        assert_eq!(
            target(".", None),
            ImportTarget::Choose(Some(std::env::current_dir().unwrap()))
        );
    }

    #[test]
    fn bounded_read_distinguishes_missing_empty_encoding_size_and_permissions() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("key");
        assert!(read(&path).unwrap_err().contains("찾을 수 없습니다"));
        std::fs::write(&path, []).unwrap();
        assert!(read(&path).unwrap_err().contains("비어"));
        std::fs::write(&path, [0xff]).unwrap();
        assert!(read(&path).unwrap_err().contains("UTF-8"));
        std::fs::write(&path, vec![b'a'; MAX_KEY_BYTES]).unwrap();
        assert_eq!(read(&path).unwrap().len(), MAX_KEY_BYTES);
        std::fs::write(&path, vec![b'a'; MAX_KEY_BYTES + 1]).unwrap();
        assert!(read(&path).unwrap_err().contains("128 KiB"));
        assert!(read_error(io::ErrorKind::PermissionDenied.into()).contains("권한"));
        assert!(read_error(io::ErrorKind::Interrupted.into()).contains("파일 상태"));
    }
}
