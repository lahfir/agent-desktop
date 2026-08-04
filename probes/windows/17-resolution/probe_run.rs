//! The measurement entry point, split from `probe.rs` so the binary root
//! stays under the repo's file-size cap.

use super::*;

/// Measures the resolution unknowns in one pass over a hosted fixture: the
/// live 0/1/N candidate counts over the fixture's own identifiers, the
/// single-element strict-resolve timing envelope (one resolve-scoped walk plus
/// the exact-match filter, per stored ref), the shared single-element read
/// cost, and a secure-field leak check that reads the password control's name
/// and value and tests them for the marker. Every timing is min-of-seven after
/// a discarded warm-up, the A15-13 methodology.
pub(crate) fn measure() -> Value {
    let apartment = win::join_multithreaded_apartment();
    let automation = match UIAutomation::new_direct() {
        Ok(automation) => automation,
        Err(error) => {
            return json!({
                "co_initialize_hresult": format!("0x{apartment:08X}"),
                "client": { "failed": failure_shape(&error) },
            });
        }
    };

    let arguments: Vec<String> = env::args().collect();
    let fd = |name: &str| -> Option<String> {
        arguments
            .iter()
            .position(|argument| argument == name)
            .and_then(|index| arguments.get(index + 1).cloned())
    };
    let attached = fd(ATTACH_FLAG);
    let (mut host, handle) = match &attached {
        Some(handle) => (None, handle.parse::<isize>().unwrap_or(0)),
        None => match spawn_host() {
            Ok((child, handle)) => (Some(child), handle),
            Err(error) => return json!({ "child_process": { "hosted": false, "error": error } }),
        },
    };

    let root = match automation.element_from_handle(Handle::from(handle)) {
        Ok(root) => root,
        Err(error) => {
            if let Some(child) = host.as_mut() {
                let _ = child.kill();
            }
            return json!({
                "child_process": { "hosted": true, "root_resolved": false, "root_failure": failure_shape(&error) },
            });
        }
    };

    let walker = match automation.get_raw_view_walker() {
        Ok(walker) => walker,
        Err(error) => {
            if let Some(child) = host.as_mut() {
                let _ = child.kill();
            }
            return json!({ "walker": { "failed": failure_shape(&error) } });
        }
    };

    let elements = measure::collect_descendants(&walker, &root, measure::WALK_DEPTH_LIMIT);
    let evidence: Vec<measure::Evidence> = elements.iter().map(measure::read_evidence).collect();

    let target = attached.as_deref().map(|_| "attached").unwrap_or("own-fixture");
    let census = measure::measure_census(&elements);

    let mut id_groups: std::collections::HashMap<(Option<String>, i32), usize> =
        std::collections::HashMap::new();
    for item in &evidence {
        *id_groups.entry((item.native_id.clone(), item.control_type)).or_insert(0) += 1;
    }
    let unique_key = id_groups
        .iter()
        .find(|((id, _), count)| id.is_some() && **count == 1)
        .map(|((id, role), _)| (id.clone().unwrap(), *role));
    let duplicate_key = id_groups
        .iter()
        .find(|((id, _), count)| id.is_some() && **count == 2)
        .map(|((id, role), _)| (id.clone().unwrap(), *role));

    let count_for = |id: &str, role: i32, name: Option<&str>| {
        evidence
            .iter()
            .filter(|item| {
                item.native_id.as_deref() == Some(id)
                    && item.control_type == role
                    && (name.is_none() || item.name.as_deref() == name)
            })
            .count()
    };
    let zero_one_n = json!({
        "unique_id_candidates": unique_key.as_ref().map(|(id, role)| count_for(id, *role, None)),
        "unique_key_present": unique_key.is_some(),
        "duplicate_id_candidates": duplicate_key.as_ref().map(|(id, role)| count_for(id, *role, None)),
        "duplicate_key_present": duplicate_key.is_some(),
        "absent_id_candidates": count_for("zz-probe-absent-id", 0, None),
    });

    let resolve_time = min_of_ms(|| {
        let mut read = 0usize;
        let _walk = measure::collect_descendants(&walker, &root, measure::WALK_DEPTH_LIMIT);
        for element in &_walk {
            let _ = measure::read_evidence(element);
            read += 1;
        }
        let _ = read;
        Ok(())
    });

    let read_time = min_of_ms(|| {
        if let Some(first) = elements.first() {
            let _ = measure::read_evidence(first);
        }
        Ok(())
    });

    let marker = "obs-pwd-marker-15ch";
    let secure = {
        let password = elements.iter().find(|element| {
            element
                .get_property_value(uiautomation::types::UIProperty::IsPassword)
                .ok()
                .and_then(|variant| measure::boolean_of(&variant))
                .unwrap_or(false)
        });
        match password {
            Some(element) => {
                let name = element
                    .get_property_value(uiautomation::types::UIProperty::Name)
                    .ok()
                    .and_then(|variant| variant.get_string().ok());
                let value = element
                    .get_property_value(uiautomation::types::UIProperty::ValueValue)
                    .ok()
                    .and_then(|variant| variant.get_string().ok());
                json!({
                    "password_control_present": true,
                    "name_contains_marker": name.map(|text| text.contains(marker)).unwrap_or(false),
                    "value_contains_marker": value.as_ref().map(|text| text.contains(marker)).unwrap_or(false),
                    "value_length": value.map(|text| text.chars().count()),
                })
            }
            None => json!({ "password_control_present": false }),
        }
    };

    let findall_pass = findall::measure_findall(&automation, &root, target);

    let document = json!({
        "child_process": {
            "hosted": attached.is_none(),
            "attached": attached.is_some(),
            "handle_int": handle,
            "root_resolved": true,
            "descendants": elements.len(),
        },
        "zero_one_n": zero_one_n,
        "secure_live_read": secure,
        "timing_ms": {
            "single_strict_resolve": {
                "min": resolve_time.0,
                "median": resolve_time.1,
                "max": resolve_time.2,
            },
            "single_element_read": {
                "min": read_time.0,
                "median": read_time.1,
                "max": read_time.2,
            },
        },
        "census": census,
        "findall_vs_walk": findall_pass,
    });

    if let Some(mut child) = host {
        let _ = child.kill();
        let _ = child.wait();
    }
    document
}
