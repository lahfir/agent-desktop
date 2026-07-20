#[derive(Debug, Clone, Default)]
pub struct WindowFilter {
    pub focused_only: bool,
    pub app: Option<String>,
}
