use super::{Scratch, scratch_nonce};
use crate::system::private_file::WindowsPrivateFile;
use crate::system::private_file::owner::{
    SidBuffer, file_owner_sid, process_token_owner_sid, process_token_user_sid_for_tests,
    require_owned_by_token_owner,
};
use agent_desktop_core::PrivateFileOps;
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
fn a_fresh_profile_artifact_inherits_system_admins_and_user_classes_and_no_users_class() {
    let profile = std::env::var_os("USERPROFILE").expect("USERPROFILE must exist on Windows");
    let fresh = Scratch::adopt(Path::new(&profile).join(format!(
        ".agent-desktop-acl-pin-{}-{:016x}",
        std::process::id(),
        scratch_nonce()
    )));
    let artifact = fresh.path().join("artifact.json");
    WindowsPrivateFile::new()
        .write_atomic(&artifact, b"{}")
        .expect("the pinned write must succeed under the user profile");

    let entries = inherited_ace_entries(&artifact);
    assert!(!entries.is_empty(), "the artifact must report ACL entries");
    for (sid, inherited) in &entries {
        assert!(
            *inherited,
            "every entry on a plain profile leaf must be inherited; {sid} is explicit"
        );
    }
    let sids: Vec<&str> = entries.iter().map(|(sid, _)| sid.as_str()).collect();
    let system_class = well_known_sid_string(WinLocalSystemSid);
    let administrators_class = well_known_sid_string(WinBuiltinAdministratorsSid);
    let users_class = well_known_sid_string(WinBuiltinUsersSid);
    let token_user = sid_string(&process_token_user_sid_for_tests().unwrap());
    let token_owner = sid_string(process_token_owner_sid().unwrap());
    assert!(
        sids.contains(&system_class.as_str()),
        "a SYSTEM-class principal must be inherited"
    );
    assert!(
        sids.contains(&administrators_class.as_str()),
        "an Administrators-class principal must be inherited"
    );
    assert!(
        sids.contains(&token_user.as_str()) || sids.contains(&token_owner.as_str()),
        "the current-user principal must be inherited"
    );
    assert!(
        !sids.contains(&users_class.as_str()),
        "no Users-class principal may appear on a plain profile leaf"
    );
}

fn inherited_ace_entries(path: &Path) -> Vec<(String, bool)> {
    let script = format!(
        "(Get-Acl -LiteralPath '{}').Access | ForEach-Object {{ \
         $_.IdentityReference.Translate([System.Security.Principal.SecurityIdentifier]).Value \
         + '|' + $_.IsInherited }}",
        path.display()
    );
    let output = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command"])
        .arg(&script)
        .output()
        .expect("powershell must be spawnable");
    assert!(
        output.status.success(),
        "Get-Acl must succeed: {}",
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

fn well_known_sid_string(kind: WELL_KNOWN_SID_TYPE) -> String {
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
    let buffer = SidBuffer::copied_from_valid(storage.as_mut_ptr().cast())
        .expect("a well-known SID must be valid");
    sid_string(&buffer)
}
