use std::os::windows::ffi::OsStrExt;
use std::path::{Component, Path, PathBuf, Prefix};

const VERBATIM: &[u16] = &[b'\\' as u16, b'\\' as u16, b'?' as u16, b'\\' as u16];
const VERBATIM_UNC: &[u16] = &[
    b'\\' as u16,
    b'\\' as u16,
    b'?' as u16,
    b'\\' as u16,
    b'U' as u16,
    b'N' as u16,
    b'C' as u16,
    b'\\' as u16,
];

pub(super) fn normalized(path: &Path) -> std::io::Result<PathBuf> {
    validate_path_nul(path)?;
    validate_components(path)?;
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        if path.has_root() || matches!(path.components().next(), Some(Component::Prefix(_))) {
            return Err(invalid_input(
                "drive-relative and root-relative Windows paths are not accepted here",
            ));
        }
        std::env::current_dir()?.join(path)
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(invalid_input(
                    "Windows private paths must not contain parent traversal",
                ));
            }
            _ => normalized.push(component),
        }
    }
    if !normalized.is_absolute() {
        return Err(invalid_input(
            "Windows path did not normalize to an absolute path",
        ));
    }
    validate_components(&normalized)?;
    Ok(normalized)
}

pub(super) fn wide_normalized(path: &Path) -> std::io::Result<Vec<u16>> {
    validate_path_nul(path)?;
    validate_components(path)?;
    let absolute: Vec<u16> = path.as_os_str().encode_wide().collect();
    let mut verbatim = if absolute.starts_with(VERBATIM) {
        absolute
    } else if absolute.starts_with(&[b'\\' as u16, b'\\' as u16]) {
        let mut path = Vec::with_capacity(VERBATIM_UNC.len() + absolute.len() - 2 + 1);
        path.extend_from_slice(VERBATIM_UNC);
        path.extend_from_slice(&absolute[2..]);
        path
    } else if is_drive_absolute(&absolute) {
        let mut path = Vec::with_capacity(VERBATIM.len() + absolute.len() + 1);
        path.extend_from_slice(VERBATIM);
        path.extend_from_slice(&absolute);
        path
    } else {
        return Err(invalid_input(
            "Windows path did not normalize to an absolute path",
        ));
    };
    if verbatim.len() + 1 > 32_767 {
        return Err(invalid_input("Windows verbatim path exceeds 32,767 units"));
    }
    verbatim.push(0);
    Ok(verbatim)
}

pub(super) fn validate_file_name(path: &Path) -> std::io::Result<()> {
    let file_name = path
        .file_name()
        .ok_or_else(|| invalid_input("private path has no filename"))?;
    validate_component(file_name)
}

fn validate_components(path: &Path) -> std::io::Result<()> {
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => validate_prefix(prefix.kind())?,
            Component::Normal(component) => validate_component(component)?,
            Component::ParentDir => {
                return Err(invalid_input(
                    "Windows private paths must not contain parent traversal",
                ));
            }
            Component::RootDir | Component::CurDir => {}
        }
    }
    Ok(())
}

fn validate_prefix(prefix: Prefix<'_>) -> std::io::Result<()> {
    match prefix {
        Prefix::Disk(_)
        | Prefix::UNC(_, _)
        | Prefix::VerbatimDisk(_)
        | Prefix::VerbatimUNC(_, _) => Ok(()),
        Prefix::DeviceNS(_) | Prefix::Verbatim(_) => Err(invalid_input(
            "Windows device namespace paths are not accepted here",
        )),
    }
}

fn validate_component(component: &std::ffi::OsStr) -> std::io::Result<()> {
    let wide: Vec<u16> = component.encode_wide().collect();
    if wide.contains(&0) {
        return Err(invalid_input("Windows paths must not contain NUL"));
    }
    if wide.contains(&(b':' as u16)) {
        return Err(invalid_input(
            "Windows alternate data streams are not accepted here",
        ));
    }
    if matches!(wide.last(), Some(last) if *last == b'.' as u16 || *last == b' ' as u16) {
        return Err(invalid_input(
            "Windows private path components must not end in a dot or space",
        ));
    }
    if is_dos_reserved(&wide) {
        return Err(invalid_input(
            "Windows DOS device basenames are not accepted here",
        ));
    }
    Ok(())
}

fn is_dos_reserved(component: &[u16]) -> bool {
    let base_end = component
        .iter()
        .position(|unit| *unit == b'.' as u16)
        .unwrap_or(component.len());
    let mut base = &component[..base_end];
    while matches!(base.last(), Some(last) if *last == b' ' as u16 || *last == b'.' as u16) {
        base = &base[..base.len() - 1];
    }
    ascii_eq(base, b"NUL")
        || ascii_eq(base, b"CON")
        || ascii_eq(base, b"AUX")
        || ascii_eq(base, b"PRN")
        || ascii_eq(base, b"CLOCK$")
        || numbered_device(base, b"COM")
        || numbered_device(base, b"LPT")
}

fn numbered_device(base: &[u16], prefix: &[u8; 3]) -> bool {
    base.len() == 4
        && ascii_eq(&base[..3], prefix)
        && matches!(base[3], unit if (b'1' as u16..=b'9' as u16).contains(&unit) || matches!(unit, 0x00b9 | 0x00b2 | 0x00b3))
}

fn ascii_eq(value: &[u16], expected: &[u8]) -> bool {
    value.len() == expected.len()
        && value.iter().zip(expected).all(|(actual, expected)| {
            u8::try_from(*actual).is_ok_and(|actual| actual.to_ascii_uppercase() == *expected)
        })
}

fn validate_path_nul(path: &Path) -> std::io::Result<()> {
    if path.as_os_str().encode_wide().any(|unit| unit == 0) {
        return Err(invalid_input("Windows paths must not contain NUL"));
    }
    Ok(())
}

fn is_drive_absolute(path: &[u16]) -> bool {
    matches!(path, [drive, colon, slash, ..] if *drive != b'\\' as u16 && *colon == b':' as u16 && *slash == b'\\' as u16)
}

fn invalid_input(message: &'static str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidInput, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_reject_windows_aliasing_syntax() {
        assert!(validate_file_name(Path::new("state:stream")).is_err());
        assert!(validate_file_name(Path::new("state.")).is_err());
        assert!(validate_file_name(Path::new("state ")).is_err());
        assert!(validate_file_name(Path::new("state.json")).is_ok());
    }

    #[test]
    fn dos_device_names_are_rejected_case_insensitively_with_extensions() {
        for name in [
            "NUL",
            "con.txt",
            "AUX.json",
            "prn",
            "CLOCK$",
            "COM1.log",
            "com9",
            "LPT1.txt",
            "lpt9",
            "COM¹.txt",
            "LPT²",
            "lpt³.json",
        ] {
            assert!(validate_file_name(Path::new(name)).is_err(), "{name}");
        }
        for name in ["COM0", "COM10", "LPT0", "LPT10", "console", "auxiliary"] {
            assert!(validate_file_name(Path::new(name)).is_ok(), "{name}");
        }
    }

    #[test]
    fn drive_absolute_detection_requires_a_root_separator() {
        assert!(is_drive_absolute(&[
            b'C' as u16,
            b':' as u16,
            b'\\' as u16,
            b'x' as u16
        ]));
        assert!(!is_drive_absolute(&[b'C' as u16, b':' as u16, b'x' as u16]));
    }

    #[test]
    fn drive_and_unc_paths_receive_verbatim_prefixes() {
        let drive = wide_normalized(Path::new(r"C:\state\refs.json")).unwrap();
        let unc = wide_normalized(Path::new(r"\\server\share\refs.json")).unwrap();

        assert!(drive.starts_with(VERBATIM));
        assert!(unc.starts_with(VERBATIM_UNC));
        assert_eq!(drive.last(), Some(&0));
        assert_eq!(unc.last(), Some(&0));
    }

    #[test]
    fn long_drive_path_is_encoded_without_legacy_max_path_truncation() {
        let path = format!(r"C:\state\{}\refs.json", "a".repeat(300));
        let encoded = wide_normalized(Path::new(&path)).unwrap();

        assert!(encoded.starts_with(VERBATIM));
        assert!(encoded.len() > 260);
    }
}
