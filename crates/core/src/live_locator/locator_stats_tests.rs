use super::LocatorStats;

#[test]
fn observation_attempts_are_serialized_and_merged() {
    let mut aggregate = LocatorStats::default();
    aggregate.reads.observation_attempts = 1;
    let mut next = LocatorStats::default();
    next.reads.observation_attempts = 2;

    aggregate.merge_observation(&next);

    assert_eq!(aggregate.reads.observation_attempts, 3);
    let json = serde_json::to_value(aggregate).unwrap();
    assert_eq!(json["reads"]["observation_attempts"], 3);
}

#[test]
fn child_label_limit_hits_are_serialized_and_merged_separately() {
    let mut aggregate = LocatorStats::default();
    aggregate.traversal.limits.child_hits = 2;
    let mut next = LocatorStats::default();
    next.traversal.limits.child_label_hits = 3;

    aggregate.merge_observation(&next);

    assert_eq!(aggregate.traversal.limits.child_hits, 2);
    assert_eq!(aggregate.traversal.limits.child_label_hits, 3);
    let json = serde_json::to_value(aggregate).unwrap();
    assert_eq!(json["traversal"]["limits"]["child_hits"], 2);
    assert_eq!(json["traversal"]["limits"]["child_label_hits"], 3);
}

#[test]
fn unclassified_native_read_failures_are_serialized_and_merged() {
    let mut aggregate = LocatorStats::default();
    let mut next = LocatorStats::default();
    next.reads.native_read_failures = 2;

    aggregate.merge_observation(&next);

    assert_eq!(aggregate.reads.native_read_failures, 2);
    let json = serde_json::to_value(aggregate).unwrap();
    assert_eq!(json["reads"]["native_read_failures"], 2);
}
