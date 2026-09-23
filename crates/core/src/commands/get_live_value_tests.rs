use super::*;

struct LiveValue(Option<String>);

impl ObservationOps for LiveValue {
    fn resolve_element_strict(
        &self,
        _entry: &RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    fn get_live_value(
        &self,
        _handle: &NativeHandle,
        _deadline: crate::Deadline,
    ) -> Result<Option<String>, AdapterError> {
        Ok(self.0.clone())
    }
}

impl ActionOps for LiveValue {}
impl InputOps for LiveValue {}
impl SystemOps for LiveValue {}

#[test]
fn get_value_does_not_resurrect_snapshot_text_when_live_value_is_absent() {
    let _guard = HomeGuard::new();
    let snapshot = save_entry(entry(vec![], Some("old text"), vec![]));
    for live in [None, Some(String::new()), Some("new text".into())] {
        let adapter = LiveValue(live.clone());
        for property in [GetProperty::Value, GetProperty::Text] {
            assert_eq!(
                get_property(&snapshot, &adapter, property).unwrap()["value"],
                json!(live)
            );
        }
    }
    assert_eq!(
        get_property(
            &snapshot,
            &LiveStateAdapter::without_live_support(),
            GetProperty::Value
        )
        .unwrap()["value"],
        "old text"
    );
}
