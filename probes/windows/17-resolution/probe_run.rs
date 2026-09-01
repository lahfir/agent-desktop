//! The measurement entry point, split from `probe.rs` so the binary root
//! stays under the repo's file-size cap.

use super::*;

/// Measures the resolution unknowns in one pass over a hosted fixture: the
/// live 0/1/N candidate counts over the fixture's own identifiers, the
/// single-element strict-resolve timing envelope (one resolve-scoped walk plus
/// the exact-match filter, per stored ref), the shared single-element read
/// cost, and the secure-field leak check in `probe_secure.rs`, which reads the
/// password control twice - once off the provider and once through the
/// adapter's own live-read composition. Every timing is min-of-seven after a
/// discarded warm-up, the A15-13 methodology.
///
/// The 0/1/N counts are resolved against a second, independent walk rather
/// than against the evidence the keys were chosen from. Counting the selected
/// keys in that same evidence would restate the selection criterion: the
/// unique key reports one and the duplicate key two however the tree actually
/// resolved, and neither number could say anything else. Against a fresh walk
/// a key that has since gone, changed identity or gained a twin reports the
/// zero or the N it now has.
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
        .and_then(|((id, role), _)| id.clone().map(|value| (value, *role)));
    let duplicate_key = id_groups
        .iter()
        .find(|((id, _), count)| id.is_some() && **count == 2)
        .and_then(|((id, role), _)| id.clone().map(|value| (value, *role)));

    let live_elements = measure::collect_descendants(&walker, &root, measure::WALK_DEPTH_LIMIT);
    let live_evidence: Vec<measure::Evidence> =
        live_elements.iter().map(measure::read_evidence).collect();
    let count_for = |id: &str, role: i32, name: Option<&str>| {
        live_evidence
            .iter()
            .filter(|item| {
                item.native_id.as_deref() == Some(id)
                    && item.control_type == role
                    && (name.is_none() || item.name.as_deref() == name)
            })
            .count()
    };
    let zero_one_n = json!({
        "resolved_against": "an independent second walk, not the map the keys were selected from",
        "selection_walk_elements": evidence.len(),
        "resolution_walk_elements": live_evidence.len(),
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

    let secure = secure::measure_secure(&elements);

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
