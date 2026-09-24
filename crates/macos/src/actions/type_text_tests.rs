use std::cell::RefCell;
use std::ops::Range;

use agent_desktop_core::{
    ActionStep, AdapterError, Deadline, DeliverySemantics, ErrorCode, RetryDisposition,
};

use super::{SemanticTextTarget, fallback_value, type_semantically};

/// An in-memory text field that records every accessibility read and write.
struct Field {
    subrole: Result<Option<String>, i32>,
    value: RefCell<Option<String>>,
    selection: Option<Range<usize>>,
    honours_selected_text: bool,
    value_settable: bool,
    value_rejection: Option<DeliverySemantics>,
    log: RefCell<Vec<String>>,
}

impl Field {
    /// A Chromium or Electron input: it accepts `AXSelectedText` but ignores
    /// it, and honours a settable `AXValue`.
    fn electron(value: &str, selection: Range<usize>) -> Self {
        Self {
            subrole: Ok(None),
            value: RefCell::new(Some(value.into())),
            selection: Some(selection),
            honours_selected_text: false,
            value_settable: true,
            value_rejection: None,
            log: RefCell::new(Vec::new()),
        }
    }

    fn record(&self, entry: String) {
        self.log.borrow_mut().push(entry);
    }

    fn log(&self) -> Vec<String> {
        self.log.borrow().clone()
    }

    fn writes(&self) -> Vec<String> {
        self.log()
            .into_iter()
            .filter(|entry| entry.starts_with("write"))
            .collect()
    }

    fn value(&self) -> Option<String> {
        self.value.borrow().clone()
    }
}

impl SemanticTextTarget for Field {
    fn read_subrole(&self, _deadline: Deadline) -> Result<Option<String>, i32> {
        self.record("read AXSubrole".into());
        self.subrole.clone()
    }

    fn read_value(&self, _deadline: Deadline) -> Option<String> {
        self.record("read AXValue".into());
        self.value()
    }

    fn read_selection(&self, _deadline: Deadline) -> Option<Range<usize>> {
        self.record("read AXSelectedTextRange".into());
        self.selection.clone()
    }

    fn value_is_settable(&self, _deadline: Deadline) -> bool {
        self.record("read AXValue settable".into());
        self.value_settable
    }

    fn write_selected_text(&self, text: &str, _deadline: Deadline) -> Result<(), AdapterError> {
        self.record(format!("write AXSelectedText={text}"));
        if self.honours_selected_text {
            let current = self.value().unwrap_or_default();
            let inserted =
                agent_desktop_core::expected_insertion(&current, text, self.selection.clone());
            self.value.replace(inserted);
        }
        Ok(())
    }

    fn write_value(&self, value: &str, _deadline: Deadline) -> Result<(), AdapterError> {
        self.record(format!("write AXValue={value}"));
        if let Some(disposition) = self.value_rejection {
            return Err(
                AdapterError::new(ErrorCode::ActionFailed, "AXValue write rejected")
                    .with_disposition(disposition),
            );
        }
        self.value.replace(Some(value.into()));
        Ok(())
    }
}

fn type_into(field: &Field, role: &str, text: &str) -> Result<Vec<ActionStep>, AdapterError> {
    type_semantically(field, role, text, Deadline::after(5_000).unwrap())
}

fn labels(steps: &[ActionStep]) -> Vec<&str> {
    steps.iter().map(ActionStep::label).collect()
}

#[test]
fn ignored_insertion_writes_the_composed_value_once() {
    let field = Field::electron("ab", 1..1);
    let steps = type_into(&field, "AXTextField", "X").unwrap();

    assert_eq!(labels(&steps), ["AXSelectedText", "AXValue"]);
    assert_eq!(
        field.writes(),
        ["write AXSelectedText=X", "write AXValue=aXb"]
    );
    assert_eq!(field.value().as_deref(), Some("aXb"));
}

#[test]
fn honoured_insertion_is_not_written_again_through_the_value() {
    let field = Field {
        honours_selected_text: true,
        ..Field::electron("ab", 1..1)
    };
    let steps = type_into(&field, "AXTextArea", "X").unwrap();

    assert_eq!(labels(&steps), ["AXSelectedText"]);
    assert_eq!(field.writes(), ["write AXSelectedText=X"]);
    assert_eq!(field.value().as_deref(), Some("aXb"));
}

#[test]
fn read_only_value_is_never_written() {
    let field = Field {
        value_settable: false,
        ..Field::electron("ab", 1..1)
    };
    let steps = type_into(&field, "AXTextField", "X").unwrap();

    assert_eq!(labels(&steps), ["AXSelectedText"]);
    assert_eq!(field.writes(), ["write AXSelectedText=X"]);
}

#[test]
fn secure_fields_are_never_read_or_written_through_the_value() {
    let masked = |role: &'static str, subrole: Result<Option<String>, i32>| {
        let field = Field {
            subrole,
            ..Field::electron("\u{2022}\u{2022}", 2..2)
        };
        let steps = type_into(&field, role, "X").unwrap();
        (labels(&steps).join(","), field.log())
    };
    let cases = [
        ("AXSecureTextField", Ok(None)),
        ("AXTextField", Ok(Some("AXSecureTextField".into()))),
        ("AXTextField", Err(-25204)),
    ];

    for (role, subrole) in cases {
        let case = format!("{role} {subrole:?}");
        let (steps, log) = masked(role, subrole);
        assert_eq!(steps, "AXSelectedText", "{case}");
        assert_eq!(log, ["read AXSubrole", "write AXSelectedText=X"], "{case}");
    }
}

#[test]
fn rejected_fallback_after_accepted_insertion_is_delivered_and_unsafe_to_retry() {
    for rejection in [
        DeliverySemantics::not_delivered(),
        DeliverySemantics::unknown(),
    ] {
        let field = Field {
            value_rejection: Some(rejection),
            ..Field::electron("ab", 1..1)
        };
        let error = type_into(&field, "AXTextField", "X").unwrap_err();

        assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
        assert_eq!(error.disposition.retry(), RetryDisposition::Unsafe);
        assert_eq!(
            field.writes(),
            ["write AXSelectedText=X", "write AXValue=aXb"]
        );
    }
}

#[test]
fn unchanged_value_falls_back_to_the_composed_value_at_the_selection() {
    assert_eq!(
        fallback_value(Some("ab"), Some(1..1), Some("ab"), "X"),
        Some("aXb".into())
    );
    assert_eq!(
        fallback_value(Some("a😀b"), Some(1..3), Some("a😀b"), "X"),
        Some("aXb".into())
    );
    assert_eq!(
        fallback_value(Some(""), None, Some(""), "new"),
        Some("new".into())
    );
}

#[test]
fn fallback_never_runs_when_the_insertion_may_have_landed_or_evidence_is_missing() {
    let cases = [
        (Some("ab"), Some(1..1), Some("aXb"), "X"),
        (Some("ab"), Some(1..1), Some("something else"), "X"),
        (None, Some(0..0), Some(""), "X"),
        (Some(""), Some(0..0), None, "X"),
        (Some("ab"), None, Some("ab"), "X"),
        (Some("ab"), Some(0..5), Some("ab"), "X"),
        (Some("😀"), Some(1..1), Some("😀"), "X"),
        (Some("ab"), Some(0..2), Some("ab"), "ab"),
        (Some("ab"), Some(1..1), Some("ab"), ""),
    ];
    for (before, range, after, text) in cases {
        assert_eq!(
            fallback_value(before, range.clone(), after, text),
            None,
            "{before:?} {range:?} {after:?} {text:?}"
        );
    }
}
