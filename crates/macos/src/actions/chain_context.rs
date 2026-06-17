pub(crate) struct ChainContext<'a> {
    pub(crate) dynamic_value: Option<&'a str>,
    pub(crate) deadline: Option<std::time::Instant>,
}
