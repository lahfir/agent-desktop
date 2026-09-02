//! The renderer's state: where the cursor is, what style it wears, and what
//! it does when a control arrives.
//!
//! Core owns the trajectory. `CursorMotion` is a function of elapsed
//! milliseconds, so this samples a clock rather than counting frames — a
//! dropped frame changes smoothness and never the instant the cursor lands,
//! which is the instant the action is waiting on.

#[cfg(target_os = "windows")]
pub(crate) use imp::SurfaceHost;

#[cfg(target_os = "windows")]
mod imp {
    use crate::system::cursor_overlay::{
        geometry, monitors, render, schedule, session_state, text, window::OverlayWindow,
    };
    use agent_desktop_core::{
        CURSOR_HIGHLIGHT_HOLD_MS, CURSOR_IDLE_REST_MS, CursorMotion, CursorOverlayControl,
        CursorOverlayStyle, CursorPhase, Point, Rect,
    };
    use std::time::{Duration, Instant};

    /// The monitor list and refresh rate read together from one display
    /// probe.
    ///
    /// They are invalidated as a pair: a resolution change, a scale change,
    /// or a monitor hot-plug makes both stale at once. `SurfaceHost::create`
    /// reads them together and `SurfaceHost::rest` re-reads them together on
    /// every idle tick for that reason.
    struct DisplayTopology {
        monitors: Vec<monitors::OverlayMonitor>,
        refresh_hz: u32,
    }

    impl DisplayTopology {
        fn probe() -> Self {
            Self {
                monitors: super::super::display_probe::monitors(),
                refresh_hz: super::super::display_probe::refresh_hz(),
            }
        }
    }

    pub(crate) struct SurfaceHost {
        window: OverlayWindow,
        session_id: String,
        style: CursorOverlayStyle,
        pose: Point,
        label: Option<String>,
        watch: session_state::EndWatch,
        topology: DisplayTopology,
    }

    impl SurfaceHost {
        pub(crate) fn create(session_id: String) -> Result<Self, agent_desktop_core::AdapterError> {
            let topology = DisplayTopology::probe();
            let pose =
                monitors::resting_point(&topology.monitors).unwrap_or(Point { x: 0.0, y: 0.0 });
            Ok(Self {
                window: OverlayWindow::create()?,
                session_id,
                style: CursorOverlayStyle::default(),
                pose,
                label: None,
                watch: session_state::EndWatch::default(),
                topology,
            })
        }

        pub(crate) fn idle_tick(&self) -> Duration {
            Duration::from_millis(CURSOR_IDLE_REST_MS / 4)
        }

        pub(crate) fn apply(&mut self, control: &CursorOverlayControl) {
            if let Some(style) = control.style() {
                self.style = style.clone();
            }
            if let Some(label) = control.label() {
                self.label = Some(label.to_owned());
            }
            match control {
                CursorOverlayControl::Enable { .. } => self.draw(0.0, None, 0.0),
                CursorOverlayControl::Hide { .. } | CursorOverlayControl::Disable { .. } => {
                    self.clear();
                }
                CursorOverlayControl::Show { .. } => self.draw(0.0, None, 0.0),
                CursorOverlayControl::Present { instruction, .. } => {
                    let destination = instruction.destination().clone();
                    let target = instruction.target().cloned();
                    match instruction.phase() {
                        CursorPhase::Travel => self.travel(destination, target),
                        CursorPhase::Effect => {
                            self.effect(destination, target, instruction.is_click())
                        }
                    }
                }
            }
        }

        /// Plays the motion core computed, then leaves the cursor at its
        /// destination. The caller acknowledges after this returns, which is
        /// what makes arrival-before-dispatch true rather than hoped for.
        fn travel(&mut self, destination: Point, target: Option<Rect>) {
            let motion = CursorMotion::new(self.pose.clone(), destination.clone())
                .with_impact(false)
                .with_ripple(false);
            let interval = schedule::frame_interval(self.topology.refresh_hz);
            let started = Instant::now();
            loop {
                let elapsed = started.elapsed().as_millis() as u64;
                let pose = motion.pose(elapsed);
                self.pose = pose.point.clone();
                self.draw(0.0, target.as_ref(), 0.0);
                if schedule::has_arrived(elapsed, motion.duration_ms()) {
                    break;
                }
                std::thread::sleep(interval);
            }
            self.pose = destination;
            self.draw(0.0, target.as_ref(), 0.0);
        }

        /// The click flourish and the outline, after dispatch has already
        /// confirmed. Fire-and-forget by contract, so nothing waits on it.
        fn effect(&mut self, destination: Point, target: Option<Rect>, click: bool) {
            self.pose = destination.clone();
            let motion = CursorMotion::new(destination.clone(), destination)
                .with_impact(click)
                .with_ripple(self.style.ripple());
            let interval = schedule::frame_interval(self.topology.refresh_hz);
            let started = Instant::now();
            loop {
                let elapsed = started.elapsed().as_millis() as u64;
                let pose = motion.pose(elapsed);
                let opacity = schedule::highlight_progress(elapsed, CURSOR_HIGHLIGHT_HOLD_MS);
                self.draw(pose.ripple, target.as_ref(), opacity);
                if elapsed >= motion.total_ms().max(CURSOR_HIGHLIGHT_HOLD_MS) {
                    break;
                }
                std::thread::sleep(interval);
            }
            self.draw(0.0, None, 0.0);
        }

        fn draw(&self, ripple_phase: f64, target: Option<&Rect>, highlight_opacity: f64) {
            let screen = monitors::monitor_for_point(&self.topology.monitors, &self.pose)
                .map(|monitor| monitor.work_area)
                .unwrap_or(Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 1920.0,
                    height: 1080.0,
                });
            let frame = render::Frame {
                tip: self.pose.clone(),
                style: &self.style,
                ripple_phase,
                target: target.cloned(),
                highlight_opacity,
                label: self.label.as_deref(),
                screen,
            };
            let mut composed = render::compose(&frame);
            if let (Some(text_rect), Some(label)) = (composed.text_rect, self.label.as_deref()) {
                text::draw_label(
                    &mut composed.surface,
                    &text_rect,
                    label,
                    self.style.rim_rgb(),
                    geometry::BUBBLE_FONT_POINTS * self.style.size(),
                );
            }
            let placement = monitors::monitor_for_point(&self.topology.monitors, &composed.origin)
                .map_or(composed.origin.clone(), |monitor| {
                    monitors::to_physical(monitor, &composed.origin)
                });
            self.window.raise();
            let _ = self.window.present(
                placement.x as i32,
                placement.y as i32,
                composed.surface.width,
                composed.surface.height,
                &composed.surface.pixels,
            );
            self.window.pump();
        }

        fn clear(&self) {
            let _ = self.window.present(0, 0, 1, 1, &[0u32]);
            self.window.pump();
        }

        /// One quiet tick: pump the window, and re-read the display
        /// topology.
        ///
        /// The monitors and refresh rate are sampled when the renderer starts
        /// and would otherwise stay fixed for its whole life, so a resolution
        /// change, a scale change or a monitor plugged in mid-session would
        /// leave every later frame mapped against a desktop that no longer
        /// exists. Re-probing on the idle tick is how that is noticed: the
        /// window procedure is a plain `DefWindowProcW` and handles no
        /// topology message, and a message handler could not reach this state
        /// anyway without sharing it across the callback boundary.
        pub(crate) fn rest(&mut self) {
            self.window.pump();
            self.topology = DisplayTopology::probe();
        }

        /// True once the session has read finished twice running.
        pub(crate) fn session_finished(&mut self) -> bool {
            let reading = session_state::classify(session_state::read_manifest(&self.session_id));
            self.watch.observe(reading)
        }
    }
}
