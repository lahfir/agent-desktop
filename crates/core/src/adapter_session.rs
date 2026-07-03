use crate::error::AdapterError;

pub trait AdapterSession: Send {
    fn close(self: Box<Self>) -> Result<(), AdapterError>;
}
