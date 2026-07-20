use std::fs::File;
#[cfg(not(windows))]
use std::fs::OpenOptions;
use std::hash::{BuildHasher, RandomState};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[cfg(windows)]
#[path = "private_file_windows.rs"]
pub(crate) mod windows;

pub(crate) fn open_private_lock(path: &Path, create: bool) -> std::io::Result<File> {
    #[cfg(windows)]
    {
        windows::open_lock(path, create)
    }
    #[cfg(not(windows))]
    {
        let parent = path
            .parent()
            .ok_or_else(|| invalid_input("private file path has no parent"))?;
        crate::private_file_parent::ensure_private(parent)?;
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(create);
        configure_unix(&mut options, 0o600);
        let file = options.open(path)?;
        validate_private_regular(&file)?;
        Ok(file)
    }
}

pub(crate) fn open_private_append(path: &Path) -> std::io::Result<File> {
    #[cfg(windows)]
    {
        windows::open_append(path)
    }
    #[cfg(not(windows))]
    {
        let mut options = OpenOptions::new();
        options.create(true).append(true);
        configure_unix(&mut options, 0o600);
        let file = options.open(path)?;
        validate_private_regular(&file)?;
        Ok(file)
    }
}

pub(crate) fn read_private_bounded(path: &Path, max_bytes: u64) -> std::io::Result<Vec<u8>> {
    let file = open_private_read(path)?;
    read_bounded(file, max_bytes)
}

pub(crate) fn read_regular_bounded(path: &Path, max_bytes: u64) -> std::io::Result<Vec<u8>> {
    #[cfg(windows)]
    let file = windows::open_regular_read(path)?;
    #[cfg(not(windows))]
    let file = {
        let mut options = OpenOptions::new();
        options.read(true);
        configure_unix(&mut options, 0);
        options.open(path)?
    };
    validate_regular(&file)?;
    validate_local_filesystem(&file)?;
    read_bounded(file, max_bytes)
}

pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    write_atomic_with(
        path,
        bytes,
        crate::private_file_parent::ensure_private,
        sync_directory,
        validate_private_destination,
    )
}

pub(crate) fn write_user_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    write_atomic_with(
        path,
        bytes,
        crate::private_file_parent::ensure_user,
        sync_user_directory,
        validate_user_destination,
    )
}

fn write_atomic_with(
    path: &Path,
    bytes: &[u8],
    ensure_parent: fn(&Path) -> std::io::Result<()>,
    sync_parent: fn(&Path) -> std::io::Result<()>,
    validate_destination: fn(&Path) -> std::io::Result<()>,
) -> std::io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| invalid_input("private file path has no parent"))?;
    ensure_parent(parent)?;
    validate_destination(path)?;
    let (temporary, mut file) = create_temporary(path)?;
    let result = (|| {
        file.write_all(bytes)?;
        file.sync_all()?;
        #[cfg(test)]
        crash_before_rename_if_requested(path);
        replace_atomic(&file, &temporary, path)?;
        validate_private_regular(&open_private_read(path)?)?;
        sync_parent(parent)
    })();
    drop(file);
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

fn validate_private_destination(path: &Path) -> std::io::Result<()> {
    match open_private_read(path) {
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn validate_user_destination(path: &Path) -> std::io::Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(invalid_input("user output path is a symlink"))
        }
        Ok(metadata) if !metadata.is_file() => {
            Err(invalid_input("user output path is not a regular file"))
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

pub(crate) fn validate_private_regular(file: &File) -> std::io::Result<std::fs::Metadata> {
    let metadata = validate_regular(file)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.uid() != unsafe { libc::geteuid() } {
            return Err(permission_denied(
                "private file is not owned by the effective user",
            ));
        }
        if metadata.mode() & 0o077 != 0 {
            return Err(permission_denied(
                "private file is accessible by group or other users",
            ));
        }
        if metadata.nlink() != 1 {
            return Err(permission_denied("private file must not be hard-linked"));
        }
    }
    #[cfg(windows)]
    windows::validate_private(file)?;
    Ok(metadata)
}

pub(crate) fn validate_regular(file: &File) -> std::io::Result<std::fs::Metadata> {
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(permission_denied("path is not a regular file"));
    }
    #[cfg(windows)]
    windows::validate_regular(file)?;
    Ok(metadata)
}

fn open_private_read(path: &Path) -> std::io::Result<File> {
    #[cfg(windows)]
    {
        windows::open_read(path)
    }
    #[cfg(not(windows))]
    {
        let mut options = OpenOptions::new();
        options.read(true);
        configure_unix(&mut options, 0);
        let file = options.open(path)?;
        validate_private_regular(&file)?;
        Ok(file)
    }
}

fn read_bounded(file: File, max_bytes: u64) -> std::io::Result<Vec<u8>> {
    let metadata = file.metadata()?;
    if metadata.len() > max_bytes {
        return Err(invalid_input("file exceeds its read limit"));
    }
    let capacity = usize::try_from(metadata.len().min(max_bytes)).unwrap_or(usize::MAX);
    let mut bytes = Vec::with_capacity(capacity);
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max_bytes {
        return Err(invalid_input("file grew beyond its read limit"));
    }
    Ok(bytes)
}

fn create_temporary(path: &Path) -> std::io::Result<(PathBuf, File)> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| invalid_input("private file path has an invalid filename"))?;
    for _ in 0..32 {
        let nonce = RandomState::new().hash_one((
            std::process::id(),
            TEMP_COUNTER.fetch_add(1, Ordering::Relaxed),
            std::time::SystemTime::now(),
        ));
        let temporary = path.with_file_name(format!(".{file_name}.{nonce:016x}.tmp"));
        #[cfg(windows)]
        match windows::create_new(&temporary) {
            Ok(file) => return Ok((temporary, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
        #[cfg(not(windows))]
        {
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            configure_unix(&mut options, 0o600);
            match options.open(&temporary) {
                Ok(file) => {
                    validate_private_regular(&file)?;
                    return Ok((temporary, file));
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "could not allocate a private temporary file",
    ))
}

#[cfg(windows)]
fn sync_directory(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

#[cfg(not(windows))]
fn sync_directory(path: &Path) -> std::io::Result<()> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW);
    }
    options.open(path)?.sync_all()
}

#[cfg(windows)]
fn sync_user_directory(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

#[cfg(not(windows))]
fn sync_user_directory(path: &Path) -> std::io::Result<()> {
    OpenOptions::new().read(true).open(path)?.sync_all()
}

#[cfg(windows)]
fn replace_atomic(file: &File, source: &Path, destination: &Path) -> std::io::Result<()> {
    windows::replace_atomic(file, source, destination)
}

#[cfg(not(windows))]
fn replace_atomic(_file: &File, source: &Path, destination: &Path) -> std::io::Result<()> {
    std::fs::rename(source, destination)
}

#[cfg(unix)]
fn configure_unix(options: &mut OpenOptions, mode: u32) {
    use std::os::unix::fs::OpenOptionsExt;
    options
        .mode(mode)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK);
}

#[cfg(not(any(unix, windows)))]
fn configure_unix(_options: &mut OpenOptions, _mode: u32) {}

#[cfg(target_os = "macos")]
fn validate_local_filesystem(file: &File) -> std::io::Result<()> {
    use std::os::fd::AsRawFd;
    let mut stats = std::mem::MaybeUninit::<libc::statfs>::uninit();
    if unsafe { libc::fstatfs(file.as_raw_fd(), stats.as_mut_ptr()) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    if unsafe { stats.assume_init() }.f_flags & libc::MNT_LOCAL as u32 == 0 {
        return Err(permission_denied(
            "network filesystems are not accepted here",
        ));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn validate_local_filesystem(file: &File) -> std::io::Result<()> {
    use std::os::fd::AsRawFd;
    let mut stats = std::mem::MaybeUninit::<libc::statfs>::uninit();
    if unsafe { libc::fstatfs(file.as_raw_fd(), stats.as_mut_ptr()) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    let fs_type = unsafe { stats.assume_init() }.f_type as u64;
    if matches!(fs_type, 0x6969 | 0x517b | 0xff53_4d42 | 0x6573_5546) {
        return Err(permission_denied(
            "network filesystems are not accepted here",
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn validate_local_filesystem(file: &File) -> std::io::Result<()> {
    windows::validate_local(file)
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
fn validate_local_filesystem(_file: &File) -> std::io::Result<()> {
    Ok(())
}

#[cfg(test)]
fn crash_before_rename_if_requested(path: &Path) {
    if std::env::var_os("AGENT_DESKTOP_TEST_CRASH_BEFORE_RENAME")
        .is_some_and(|value| Path::new(&value) == path)
    {
        std::process::abort();
    }
}

pub(super) fn invalid_input(message: &'static str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, message)
}

pub(super) fn permission_denied(message: &'static str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::PermissionDenied, message)
}

#[cfg(test)]
#[path = "private_file_tests.rs"]
mod tests;
