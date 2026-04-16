use crate::convert::string::c_to_string;
use crate::types::AdNotificationFilter;
use agent_desktop_core::notification::NotificationFilter;

/// Converts a C `AdNotificationFilter` into the core filter type.
/// Null pointers become `None`; `has_limit == false` clears the limit.
///
/// # Safety
/// `filter` must be null or point to a valid `AdNotificationFilter`.
/// The embedded C strings must outlive this call.
pub(crate) unsafe fn filter_from_c(filter: *const AdNotificationFilter) -> NotificationFilter {
    if filter.is_null() {
        return NotificationFilter::default();
    }
    let f: &AdNotificationFilter = unsafe { &*filter };
    NotificationFilter {
        app: unsafe { c_to_string(f.app) },
        text: unsafe { c_to_string(f.text) },
        limit: if f.has_limit {
            Some(f.limit as usize)
        } else {
            None
        },
    }
}
