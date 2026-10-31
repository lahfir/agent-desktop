use super::read_text;
use std::io::Cursor;

#[test]
fn stdin_text_preserves_empty_input_unicode_crlf_and_trailing_newlines() {
    assert_eq!(read_text(Cursor::new(b"")).unwrap(), "");
    let text = "first\r\nsecond\n世界";
    assert_eq!(read_text(Cursor::new(text.as_bytes())).unwrap(), text);
}

#[test]
fn stdin_text_accepts_the_limit_and_rejects_one_extra_byte() {
    let input = vec![b'x'; 128 * 1024];
    assert_eq!(read_text(Cursor::new(input)).unwrap().len(), 128 * 1024);
    let error = read_text(Cursor::new(vec![b'x'; 128 * 1024 + 1])).unwrap_err();
    assert!(error.to_string().contains("128 KiB"));
}

#[test]
fn stdin_text_rejects_invalid_utf8_and_nul_without_echoing_input() {
    for input in [b"private-marker\xff".as_slice(), b"private-marker\0"] {
        let error = read_text(Cursor::new(input)).unwrap_err();
        assert_eq!(error.code(), "INVALID_ARGS");
        assert!(!error.to_string().contains("private-marker"));
    }
}

#[test]
fn stdin_read_failures_are_generic() {
    struct Failure;

    impl std::io::Read for Failure {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("private marker"))
        }
    }

    let error = read_text(Failure).unwrap_err();
    assert_eq!(error.code(), "INVALID_ARGS");
    assert!(!error.to_string().contains("private marker"));
}

#[test]
fn stdin_flags_conflict_with_other_input_sources() {
    use clap::Parser;
    for args in [
        vec!["ad", "type", "@s8f3k2p9:e1"],
        vec!["ad", "set-value", "@s8f3k2p9:e1"],
        vec!["ad", "clipboard-set"],
    ] {
        let parse = |extra: &[&str]| crate::Cli::try_parse_from(args.iter().chain(extra).copied());
        let mut command = parse(&["--stdin"]).unwrap().command.unwrap();
        assert!(super::input(&mut command).is_some());
        assert!(parse(&["text", "--stdin"]).is_err());
        let mut literal = parse(&["--", "--stdin"]).unwrap().command.unwrap();
        assert!(super::input(&mut literal).is_none());
        if args[1] == "clipboard-set" {
            for flag in ["--image", "--file-url"] {
                assert!(parse(&["--stdin", flag, "file"]).is_err());
            }
        } else {
            assert!(parse(&[]).is_err());
        }
    }
}
