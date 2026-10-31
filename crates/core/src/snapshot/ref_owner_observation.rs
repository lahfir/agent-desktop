use crate::{SnapshotOwner, system::process_state::ProcessState};

pub(crate) enum RefOwnerObservation {
    Present(SnapshotOwner),
    Absent(Option<ProcessState>),
    Unavailable(ProcessState),
}
