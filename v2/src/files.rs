//! Files owned individually, never recursive cleanup. Saves are no-clobber atomic publishes.
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};
use uuid::Uuid;
const MAX_BYTES: usize = 2 * 1024 * 1024;
pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn directory(path: &Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err("Expected absolute private directory".into());
    }
    // Refuse symlink components, including Downloads redirection, rather than following links.
    let mut p = PathBuf::new();
    for component in path.components() {
        p.push(component);
        match fs::symlink_metadata(&p) {
            Ok(m) if m.is_dir() && !m.file_type().is_symlink() => {}
            Ok(_) => return Err("Symlink/non-directory path rejected".into()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&p).map_err(|_| "Cannot create output directory")?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    fs::set_permissions(&p, fs::Permissions::from_mode(0o700))
                        .map_err(|_| "Cannot restrict directory")?;
                }
            }
            Err(_) => return Err("Cannot inspect output directory".into()),
        }
    }
    Ok(())
}
#[derive(Debug)]
pub struct OwnedFile {
    path: PathBuf,
    hash: String,
    len: usize,
    identity: File,
    remove: bool,
}
impl OwnedFile {
    pub fn create(dir: &Path, extension: &str, bytes: &[u8]) -> Result<Self, String> {
        directory(dir)?;
        if bytes.len() > MAX_BYTES {
            return Err("Output exceeds 2 MiB".into());
        }
        let path = dir.join(format!("tezbar-qr-{}.{}", Uuid::new_v4(), extension));
        let mut options = OpenOptions::new();
        options.write(true).read(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options
            .open(&path)
            .map_err(|_| "Cannot create private output")?;
        let mut owned = Self {
            path,
            hash: digest(bytes),
            len: bytes.len(),
            identity: file,
            remove: true,
        };
        owned
            .identity
            .write_all(bytes)
            .map_err(|_| "Cannot write output")?;
        owned
            .identity
            .sync_all()
            .map_err(|_| "Cannot flush output")?;
        Ok(owned)
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn retain(&mut self) {
        self.remove = false;
    }
    pub fn cleanup_after_ack(&mut self) {
        self.remove = true;
    }
    fn still_owned(&self) -> bool {
        let Ok(m) = fs::symlink_metadata(&self.path) else {
            return false;
        };
        if !m.is_file() || m.file_type().is_symlink() || m.len() != self.len as u64 {
            return false;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let Ok(original) = self.identity.metadata() else {
                return false;
            };
            if m.dev() != original.dev() || m.ino() != original.ino() {
                return false;
            }
        }
        let Ok(mut f) = File::open(&self.path) else {
            return false;
        };
        let Ok(original_handle) = self
            .identity
            .try_clone()
            .and_then(same_file::Handle::from_file)
        else {
            return false;
        };
        let Ok(current_handle) = f.try_clone().and_then(same_file::Handle::from_file) else {
            return false;
        };
        if original_handle != current_handle {
            return false;
        }
        let mut data = Vec::new();
        std::io::Read::by_ref(&mut f)
            .take((MAX_BYTES + 1) as u64)
            .read_to_end(&mut data)
            .is_ok()
            && digest(&data) == self.hash
    }
}
impl Drop for OwnedFile {
    fn drop(&mut self) {
        if self.remove && self.still_owned() {
            let _ = fs::remove_file(&self.path);
        }
    }
}
pub fn save(dir: &Path, name: &str, bytes: &[u8]) -> Result<PathBuf, String> {
    if name.contains(['/', '\\']) || !name.starts_with("qrcode-") {
        return Err("Unsafe filename".into());
    }
    directory(dir)?;
    let destination = dir.join(name);
    let stage = OwnedFile::create(dir, "tmp", bytes)?;
    // Atomic no-clobber hard link. No rename overwrite, no link following of destination.
    // Staging name is never re-used as the destination; guard only owns its original inode.
    fs::hard_link(stage.path(), &destination).map_err(|e| {
        if e.kind() == std::io::ErrorKind::AlreadyExists {
            "File already exists; existing file was preserved".to_owned()
        } else {
            "Cannot publish output file".into()
        }
    })?;
    // Removing owned staging link cannot remove the published link or a replacement at the old name.
    drop(stage);
    Ok(destination)
}
// All adapters/sessions in this process share the scan + create critical section.
// Existing unknown/unacknowledged entries are counted, never removed to make room.
static CLIPBOARD_STAGING: Mutex<()> = Mutex::new(());
pub fn stage_clipboard(dir: &Path, bytes: &[u8]) -> Result<OwnedFile, String> {
    let _reservation = CLIPBOARD_STAGING
        .lock()
        .map_err(|_| "Clipboard staging lock unavailable")?;
    if bytes.len() > MAX_BYTES {
        return Err("Output exceeds 2 MiB".into());
    }
    directory(dir)?;
    let mut count = 0usize;
    let mut size = 0u64;
    for entry in fs::read_dir(dir)
        .map_err(|_| "Cannot inspect clipboard staging")?
        .take(130)
    {
        let entry = entry.map_err(|_| "Cannot inspect clipboard staging")?;
        let m = entry
            .file_type()
            .map_err(|_| "Cannot inspect clipboard staging")?;
        if !m.is_file() {
            return Err("Unexpected clipboard staging entry".into());
        }
        count += 1;
        size = size.saturating_add(
            entry
                .metadata()
                .map_err(|_| "Cannot inspect clipboard staging")?
                .len(),
        );
    }
    if count >= 128 || size.saturating_add(bytes.len() as u64) > 128 * MAX_BYTES as u64 {
        return Err(
            "Clipboard staging is full of unacknowledged files; manual review required".into(),
        );
    }
    OwnedFile::create(dir, "png", bytes)
}
