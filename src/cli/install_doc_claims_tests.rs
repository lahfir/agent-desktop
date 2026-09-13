const README: &str = include_str!("../../README.md");

const INSTALL_HEADING: &str = "## Installation";
const NEXT_TOP_HEADING: &str = "## Language bindings";

/// The README is committed with CRLF endings, so the section is bounded by
/// heading text alone rather than by a line terminator the working tree may
/// rewrite.
fn install_section() -> &'static str {
    let start = README
        .find(INSTALL_HEADING)
        .expect("the README must carry an Installation section");
    let end = README[start..]
        .find(NEXT_TOP_HEADING)
        .map_or(README.len(), |offset| start + offset);
    &README[start..end]
}

/// The install path's two residual risks are closed as documentation rather
/// than as code, which only holds while the documentation says the two things
/// that close them. Same-origin checksums detect corruption but do not prove
/// provenance, so the reader needs the attestation command; and a modern npm
/// blocks the postinstall that fetches the binary, so the reader needs the
/// allowlist entry before the wrapper's binary-not-found failure is their
/// first experience of the tool.
#[test]
fn the_install_section_closes_both_documented_npm_risks() {
    let section = install_section();

    assert!(
        section.contains("gh attestation verify"),
        "the install section must show the attestation command: the published \
         checksums share an origin with the artifact they describe, so they \
         detect corruption and prove nothing about provenance"
    );
    assert!(
        section.contains("allowScripts"),
        "the install section must publish the allowScripts configuration: npm \
         blocks the postinstall that fetches the native binary, and without \
         the allowlist entry a Windows reader meets a binary-not-found failure \
         instead of a working install"
    );
}
