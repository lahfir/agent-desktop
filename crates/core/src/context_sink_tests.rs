use super::*;
use crate::{TraceArtifact, TraceArtifactKind, TraceArtifactLimits, TraceSink};
use std::sync::{Arc, Mutex};

#[derive(Debug, Default)]
struct RecordingSink {
    events: Mutex<Vec<Value>>,
    artifacts: Mutex<Vec<(TraceArtifactKind, String, Vec<u8>)>>,
    fail: bool,
}

impl TraceSink for RecordingSink {
    fn append(
        &self,
        event: &Value,
        _deadline: crate::Deadline,
        _byte_budget: u64,
    ) -> Result<(), AppError> {
        if self.fail {
            return Err(AppError::Internal("unavailable".into()));
        }
        self.events.lock().unwrap().push(event.clone());
        Ok(())
    }

    fn store_artifact(
        &self,
        artifact: TraceArtifact<'_>,
        _limits: TraceArtifactLimits,
    ) -> Result<String, AppError> {
        if self.fail {
            return Err(AppError::Internal("unavailable".into()));
        }
        self.artifacts.lock().unwrap().push((
            artifact.kind,
            artifact.name.into(),
            artifact.bytes.into(),
        ));
        Ok(format!("opaque:{}", artifact.name))
    }
}

#[test]
fn native_sink_receives_sanitized_events_and_inherited_child_without_files() {
    let sink = Arc::new(RecordingSink::default());
    let context = CommandContext::new_with_trace_sink(None, sink.clone(), true, true).unwrap();
    let child = context.for_batch_item(None).unwrap();
    child
        .trace("fixture.event", json!({"password": "hidden", "count": 1}))
        .unwrap();
    let events = sink.events.lock().unwrap();
    let event = events
        .iter()
        .find(|event| event["event"] == "fixture.event")
        .unwrap();
    assert!(!event.to_string().contains("hidden"));
    assert_eq!(event["count"], 1);
    assert!(event["seq"].is_u64());
    assert!(event["writer_pid"].is_u64());
    assert!(child.trace_enabled());
    assert!(!format!("{child:?}").contains("RecordingSink"));
}

#[test]
fn strict_native_sink_failure_preserves_post_action_delivery() {
    let sink = Arc::new(RecordingSink {
        fail: true,
        ..Default::default()
    });
    let context = CommandContext::new_with_trace_sink(None, sink, true, false).unwrap();
    let error = trace_error_with_disposition(
        context
            .trace("command.end", json!({"ok": true}))
            .unwrap_err(),
        crate::DeliverySemantics::delivered_unverified(),
    );
    let AppError::Adapter(error) = error else {
        panic!("adapter delivery expected");
    };
    assert_eq!(
        error.disposition,
        crate::DeliverySemantics::delivered_unverified()
    );
    assert!(context.trace_enabled());
}

#[test]
fn native_sink_artifact_failure_is_returned_without_local_fallback() {
    let context = CommandContext::new_with_trace_sink(
        None,
        Arc::new(RecordingSink {
            fail: true,
            ..Default::default()
        }),
        true,
        true,
    )
    .unwrap();
    assert!(
        context
            .trace_artifact(TraceArtifact {
                kind: TraceArtifactKind::Screenshot,
                name: "fixture.png",
                bytes: b"png",
                deadline: crate::Deadline::standard().unwrap(),
            })
            .unwrap()
            .is_err()
    );
}

#[test]
fn native_sink_rejects_oversized_events_before_append() {
    let sink = Arc::new(RecordingSink::default());
    let context = CommandContext::new_with_trace_sink(None, sink.clone(), true, false).unwrap();
    assert!(
        context
            .trace("fixture", json!({"data": "x".repeat(1024 * 1024)}))
            .is_err()
    );
    assert!(
        !sink
            .events
            .lock()
            .unwrap()
            .iter()
            .any(|event| event["event"] == "fixture")
    );
}

#[test]
fn nonstrict_native_sink_errors_do_not_disable_sink_or_open_files() {
    let context = CommandContext::new_with_trace_sink(
        None,
        Arc::new(RecordingSink {
            fail: true,
            ..Default::default()
        }),
        false,
        false,
    )
    .unwrap();
    context.trace("fixture", json!({})).unwrap();
    let child = context.for_batch_item(None).unwrap();
    child.trace("fixture", json!({})).unwrap();
    assert!(child.trace_enabled());
}

#[test]
fn independent_native_sink_handles_do_not_mix_authority() {
    let first = Arc::new(RecordingSink::default());
    let second = Arc::new(RecordingSink::default());
    let first_context =
        CommandContext::new_with_trace_sink(None, first.clone(), true, false).unwrap();
    let second_context =
        CommandContext::new_with_trace_sink(None, second.clone(), true, false).unwrap();
    std::thread::scope(|threads| {
        threads.spawn(|| first_context.trace("first", json!({"count": 1})).unwrap());
        threads.spawn(|| second_context.trace("second", json!({"count": 2})).unwrap());
    });
    assert!(
        !first
            .events
            .lock()
            .unwrap()
            .iter()
            .any(|event| event["event"] == "second")
    );
    assert!(
        !second
            .events
            .lock()
            .unwrap()
            .iter()
            .any(|event| event["event"] == "first")
    );
}

#[test]
fn native_sink_can_wait_for_ack_from_independently_driven_worker() {
    #[derive(Debug)]
    struct AcknowledgedSink(std::sync::mpsc::Sender<(Value, std::sync::mpsc::SyncSender<()>)>);
    impl TraceSink for AcknowledgedSink {
        fn append(
            &self,
            event: &Value,
            deadline: crate::Deadline,
            _budget: u64,
        ) -> Result<(), AppError> {
            let (sender, receiver) = std::sync::mpsc::sync_channel(1);
            self.0
                .send((event.clone(), sender))
                .map_err(|_| AppError::Internal("disconnected".into()))?;
            receiver
                .recv_timeout(deadline.remaining())
                .map_err(|_| AppError::Internal("ack unavailable".into()))?;
            Ok(())
        }
    }
    let (sender, receiver) = std::sync::mpsc::channel::<(Value, std::sync::mpsc::SyncSender<()>)>();
    let worker = std::thread::spawn(move || {
        let mut records = Vec::new();
        for (event, ack) in receiver {
            records.push(event);
            ack.send(()).unwrap();
        }
        records
    });
    {
        let context = CommandContext::new_with_trace_sink(
            None,
            Arc::new(AcknowledgedSink(sender)),
            true,
            false,
        )
        .unwrap();
        context.trace("acknowledged", json!({})).unwrap();
    }
    let records = worker.join().unwrap();
    assert_eq!(records.len(), 2);
    assert_eq!(records[1]["event"], "acknowledged");
}

#[test]
fn event_only_native_sink_refuses_artifacts_explicitly() {
    #[derive(Debug)]
    struct EventOnly;
    impl TraceSink for EventOnly {
        fn append(
            &self,
            _event: &Value,
            _deadline: crate::Deadline,
            _budget: u64,
        ) -> Result<(), AppError> {
            Ok(())
        }
    }
    let context =
        CommandContext::new_with_trace_sink(None, Arc::new(EventOnly), true, true).unwrap();
    assert!(
        context
            .trace_artifact(TraceArtifact {
                kind: TraceArtifactKind::RefMap,
                name: "fixture.json",
                bytes: b"{}",
                deadline: crate::Deadline::standard().unwrap(),
            })
            .unwrap()
            .is_err()
    );
}

#[test]
fn native_sink_rejects_invalid_artifact_reference() {
    #[derive(Debug)]
    struct InvalidReference;
    impl TraceSink for InvalidReference {
        fn append(
            &self,
            _event: &Value,
            _deadline: crate::Deadline,
            _budget: u64,
        ) -> Result<(), AppError> {
            Ok(())
        }
        fn store_artifact(
            &self,
            _artifact: TraceArtifact<'_>,
            _limits: TraceArtifactLimits,
        ) -> Result<String, AppError> {
            Ok("invalid\nreference".into())
        }
    }
    let context =
        CommandContext::new_with_trace_sink(None, Arc::new(InvalidReference), true, true).unwrap();
    assert!(
        context
            .trace_artifact(TraceArtifact {
                kind: TraceArtifactKind::RefMap,
                name: "fixture.json",
                bytes: b"{}",
                deadline: crate::Deadline::standard().unwrap(),
            })
            .unwrap()
            .is_err()
    );
}
