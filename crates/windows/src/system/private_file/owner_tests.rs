use super::{Scratch, scratch_nonce};
use crate::system::private_file::WindowsPrivateFile;
use crate::system::private_file::owner::forced_foreign_owner::with_forced_foreign_owner;
use crate::system::private_file::owner::{
    SidBuffer, file_owner_sid, process_token_owner_sid, process_token_user_sid_for_tests,
    require_owned_by_token_owner,
};
use agent_desktop_core::PrivateFileOps;
use std::io::ErrorKind;
use std::path::Path;
use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Security::Authorization::ConvertSidToStringSidW;
use windows_sys::Win32::Security::{
    CreateWellKnownSid, SECURITY_MAX_SID_SIZE, WELL_KNOWN_SID_TYPE, WinBuiltinAdministratorsSid,
    WinBuiltinUsersSid, WinLocalSystemSid,
};

#[test]
fn a_freshly_created_files_owner_equals_the_process_token_owner() {
    let scratch = Scratch::new("owner-fresh");
    let path = scratch.path().join("fresh.txt");
    let file = std::fs::File::create(&path).unwrap();

    let owner_sid = file_owner_sid(&file).unwrap();
    let token_owner = process_token_owner_sid().unwrap();

    assert!(
        token_owner.matches(&owner_sid),
        "a freshly created file must land owned by the token owner"
    );
    require_owned_by_token_owner(&file, "freshly created file").unwrap();
}

#[test]
fn the_token_owner_does_not_match_a_foreign_well_known_principal() {
    let token_owner = process_token_owner_sid().unwrap();
    let foreign = well_known_sid_buffer(WinLocalSystemSid);

    assert!(
        !token_owner.matches(&foreign),
        "the LocalSystem principal must be foreign to this test process's token owner"
    );
}

#[test]
fn require_owned_by_token_owner_refuses_a_foreign_owner_via_the_forced_seam() {
    let scratch = Scratch::new("owner-foreign");
    let path = scratch.path().join("fresh.txt");
    let file = std::fs::File::create(&path).unwrap();

    let refused = with_forced_foreign_owner(|| {
        require_owned_by_token_owner(&file, "seam target").unwrap_err()
    });

    assert_eq!(refused.kind(), ErrorKind::PermissionDenied);
    assert!(
        refused.to_string().contains("foreign principal"),
        "the refusal must name the foreign principal: {refused}"
    );
}

#[test]
fn write_atomic_refuses_when_the_owner_seam_forces_a_foreign_principal() {
    let scratch = Scratch::new("owner-foreign-write");
    let artifact = scratch.path().join("artifact.json");
    let ops = WindowsPrivateFile::new();

    let refused = with_forced_foreign_owner(|| ops.write_atomic(&artifact, b"secret").unwrap_err());

    assert_eq!(refused.kind(), ErrorKind::PermissionDenied);
    assert!(
        refused.to_string().contains("foreign principal"),
        "the refused write must name the foreign principal: {refused}"
    );
    assert!(
        !artifact.exists(),
        "no artifact may land when ownership validation refuses the write"
    );
}

#[test]
fn read_private_bounded_refuses_when_the_owner_seam_forces_a_foreign_principal() {
    let scratch = Scratch::new("owner-foreign-read");
    let artifact = scratch.path().join("artifact.json");
    let ops = WindowsPrivateFile::new();
    ops.write_atomic(&artifact, b"payload").unwrap();

    let refused =
        with_forced_foreign_owner(|| ops.read_private_bounded(&artifact, 64).unwrap_err());

    assert_eq!(refused.kind(), ErrorKind::PermissionDenied);
    assert!(
        refused.to_string().contains("foreign principal"),
        "the refused read must name the foreign principal: {refused}"
    );
}

#[test]
fn copied_from_valid_rejects_a_null_and_an_oversized_sid_with_invalid_data() {
    let null_kind = SidBuffer::copied_from_valid(std::ptr::null_mut())
        .err()
        .map(|error| error.kind());
    assert_eq!(null_kind, Some(ErrorKind::InvalidData));

    let mut oversized = [0_u8; 16];
    oversized[0] = 1;
    oversized[1] = 200;
    let oversized_kind = SidBuffer::copied_from_valid(oversized.as_mut_ptr().cast())
        .err()
        .map(|error| error.kind());
    assert_eq!(oversized_kind, Some(ErrorKind::InvalidData));
}

#[test]
fn a_profile_artifact_keeps_system_admins_and_user_and_never_users_across_create_and_replace() {
    let local_app_data =
        std::env::var_os("LOCALAPPDATA").expect("LOCALAPPDATA must exist on Windows");
    let fresh = Scratch::adopt(Path::new(&local_app_data).join("Temp").join(format!(
        ".agent-desktop-acl-pin-{}-{:016x}",
        std::process::id(),
        scratch_nonce()
    )));
    let artifact = fresh.path().join("artifact.json");
    let ops = WindowsPrivateFile::new();

    ops.write_atomic(&artifact, b"{}")
        .expect("the pinned create-path write must succeed under the user profile");
    let created = inherited_ace_entries(&artifact);
    assert_every_ace_is_inherited(&created);
    assert_profile_security_principals(&created);

    assert!(
        artifact.exists(),
        "the create-path write must leave a destination for the replace path to overwrite"
    );
    ops.write_atomic(&artifact, b"{\"v\":2}")
        .expect("the pinned replace-path write must succeed over the pre-existing leaf");
    assert_profile_security_principals(&inherited_ace_entries(&artifact));
}

fn assert_every_ace_is_inherited(entries: &[(String, bool)]) {
    assert!(!entries.is_empty(), "the artifact must report ACL entries");
    for (sid, inherited) in entries {
        assert!(
            *inherited,
            "every entry on a freshly created profile leaf must be inherited; {sid} is explicit"
        );
    }
}

fn assert_profile_security_principals(entries: &[(String, bool)]) {
    assert!(!entries.is_empty(), "the artifact must report ACL entries");
    let sids: Vec<&str> = entries.iter().map(|(sid, _)| sid.as_str()).collect();
    let system_class = well_known_sid_string(WinLocalSystemSid);
    let administrators_class = well_known_sid_string(WinBuiltinAdministratorsSid);
    let users_class = well_known_sid_string(WinBuiltinUsersSid);
    let token_user = sid_string(&process_token_user_sid_for_tests().unwrap());
    let token_owner = sid_string(process_token_owner_sid().unwrap());
    assert!(
        sids.contains(&system_class.as_str()),
        "a SYSTEM-class principal must be present"
    );
    assert!(
        sids.contains(&administrators_class.as_str()),
        "an Administrators-class principal must be present"
    );
    assert!(
        sids.contains(&token_user.as_str()) || sids.contains(&token_owner.as_str()),
        "the current-user principal must be present"
    );
    assert!(
        !sids.contains(&users_class.as_str()),
        "no Users-class principal may appear on a profile leaf"
    );
}

fn inherited_ace_entries(path: &Path) -> Vec<(String, bool)> {
    let script = format!(
        "$rules = [System.IO.FileInfo]::new('{}').GetAccessControl('Access')\
         .GetAccessRules($true, $true, [System.Security.Principal.SecurityIdentifier]); \
         foreach ($rule in $rules) {{ \
         [Console]::WriteLine(($rule.IdentityReference.Value + '|' + $rule.IsInherited)) }}",
        path.display()
    );
    let output = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command"])
        .arg(&script)
        .output()
        .expect("powershell must be spawnable");
    assert!(
        output.status.success(),
        "the module-free acl read must succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| {
            let (sid, inherited) = line
                .split_once('|')
                .expect("each line must be sid|inherited");
            (sid.to_string(), inherited.eq_ignore_ascii_case("true"))
        })
        .collect()
}

fn sid_string(sid: &SidBuffer) -> String {
    let mut text: windows_sys::core::PWSTR = std::ptr::null_mut();
    let converted = unsafe { ConvertSidToStringSidW(sid.as_psid(), &mut text) };
    assert!(converted != 0, "the SID must convert to its string form");
    let mut length = 0_usize;
    while unsafe { *text.add(length) } != 0 {
        length += 1;
    }
    let value = String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(text, length) });
    unsafe { LocalFree(text.cast()) };
    value
}

fn well_known_sid_buffer(kind: WELL_KNOWN_SID_TYPE) -> SidBuffer {
    let mut storage = [0_u64; 9];
    let mut size: u32 = SECURITY_MAX_SID_SIZE;
    let created = unsafe {
        CreateWellKnownSid(
            kind,
            std::ptr::null_mut(),
            storage.as_mut_ptr().cast(),
            &mut size,
        )
    };
    assert!(
        created != 0,
        "CreateWellKnownSid must succeed for kind {kind}"
    );
    SidBuffer::copied_from_valid(storage.as_mut_ptr().cast())
        .expect("a well-known SID must be valid")
}

fn well_known_sid_string(kind: WELL_KNOWN_SID_TYPE) -> String {
    sid_string(&well_known_sid_buffer(kind))
}
