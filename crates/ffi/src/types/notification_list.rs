use crate::types::notification_info::AdNotificationInfo;

pub struct AdNotificationList {
    #[allow(dead_code)] // populated + read by the Unit 8 notifications module
    pub(crate) items: Box<[AdNotificationInfo]>,
}
