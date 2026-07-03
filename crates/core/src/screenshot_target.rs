pub enum ScreenshotTarget {
    Screen(usize),
    /// Capture the largest visible window owned by this process ID.
    Window(i32),
    FullScreen,
}
