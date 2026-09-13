use super::{Frame, compose};
use agent_desktop_core::{CursorOverlayStyle, Point, Rect};

fn screen() -> Rect {
    Rect {
        x: 0.0,
        y: 0.0,
        width: 1608.0,
        height: 780.0,
    }
}

fn frame<'a>(style: &'a CursorOverlayStyle, label: Option<&'a str>) -> Frame<'a> {
    Frame {
        tip: Point { x: 400.0, y: 300.0 },
        style,
        ripple_phase: 0.0,
        target: None,
        highlight_opacity: 0.0,
        label,
        screen: screen(),
    }
}

/// Whether this host is one whose frame timings mean anything.
///
/// The measurement is always printed; the assertion is opt-in. A wall-clock
/// budget describes hardware, and a shared CI runner is not hardware anyone
/// chose: this bound was briefly asserted on every lane and the hosted ARM64
/// runner failed it on timing alone, which is how a real budget gets loosened
/// into one that can no longer fire.
///
/// So it is gated the way every other performance number on this platform is.
/// The macOS baseline script cannot run here - it opens an `.app` bundle - and
/// the probe corpus measures cost locally, min-of-seven with the warm-up
/// discarded, on a quiesced desktop. This budget belongs to that same run:
/// export `AGENT_DESKTOP_PERF_BUDGET=1` on a controlled host and it is
/// enforced; everywhere else the numbers are reported so a developer watching
/// them climb still sees it.
fn budget_is_enforced() -> bool {
    std::env::var_os("AGENT_DESKTOP_PERF_BUDGET").is_some()
}

/// Scales a shipped-binary budget to the profile running, so the same number
/// can be stated once. An unoptimised build measures roughly 2.5x slower.
fn budget(release_millis: u64) -> std::time::Duration {
    let scale = if cfg!(debug_assertions) { 3 } else { 1 };
    std::time::Duration::from_millis(release_millis * scale)
}

fn painted_pixels(composed: &super::Composed) -> usize {
    composed
        .surface
        .pixels
        .iter()
        .filter(|pixel| (*pixel >> 24) > 0)
        .count()
}

#[test]
fn a_bare_frame_draws_the_cursor_and_nothing_else() {
    let style = CursorOverlayStyle::default();

    let composed = compose(&frame(&style, None));

    assert!(painted_pixels(&composed) > 0, "the cursor is drawn");
    assert!(composed.text_rect.is_none(), "no label, no text rectangle");
}

/// The style's effect switches are the operator's, so turning them off has to
/// actually remove the pixels rather than merely skip a branch.
#[test]
fn disabling_the_ripple_removes_it_while_the_cursor_stays() {
    let mut off = CursorOverlayStyle::default();
    off.set_effects(false, true);
    let on = CursorOverlayStyle::default();

    let mut with_ripple = frame(&on, None);
    with_ripple.ripple_phase = 0.5;
    let mut without = frame(&off, None);
    without.ripple_phase = 0.5;

    let drawn = compose(&with_ripple);
    let suppressed = compose(&without);

    assert!(painted_pixels(&drawn) > painted_pixels(&suppressed));
    assert!(
        painted_pixels(&suppressed) > 0,
        "the cursor itself is not a ripple and must survive"
    );
}

#[test]
fn disabling_the_highlight_removes_it() {
    let mut off = CursorOverlayStyle::default();
    off.set_effects(true, false);
    let on = CursorOverlayStyle::default();
    let target = Rect {
        x: 380.0,
        y: 280.0,
        width: 120.0,
        height: 40.0,
    };

    let mut shown = frame(&on, None);
    shown.target = Some(target);
    shown.highlight_opacity = 1.0;
    let mut hidden = frame(&off, None);
    hidden.target = Some(target);
    hidden.highlight_opacity = 1.0;

    assert!(painted_pixels(&compose(&shown)) > painted_pixels(&compose(&hidden)));
}

/// The label's rectangle is where GDI is allowed to write, and it has to sit
/// inside the surface or the text lands outside the window entirely.
#[test]
fn a_label_yields_a_text_rectangle_inside_the_surface() {
    let style = CursorOverlayStyle::default();

    let composed = compose(&frame(&style, Some("open the file")));

    let text = composed.text_rect.expect("a label yields a text rectangle");
    assert!(text.x >= 0.0 && text.y >= 0.0);
    assert!(text.x + text.width <= f64::from(composed.surface.width));
    assert!(text.y + text.height <= f64::from(composed.surface.height));
    assert!(text.width > 0.0 && text.height > 0.0);
}

/// The bubble body must be opaque under the text, because that is what makes
/// forcing the text rectangle's alpha correct rather than a workaround.
#[test]
fn the_bubble_body_is_opaque_beneath_where_the_text_will_go() {
    let style = CursorOverlayStyle::default();

    let composed = compose(&frame(&style, Some("open the file")));
    let text = composed.text_rect.expect("a text rectangle");

    let alpha = composed.surface.alpha_at(
        (text.x + text.width / 2.0) as i32,
        (text.y + text.height / 2.0) as i32,
    );
    assert_eq!(
        alpha, 255,
        "GDI writes RGB without alpha, so it may only draw where the body is already opaque"
    );
}

/// A destination near the screen edge must not push the bubble off it: core
/// places the label, and the surface has to follow that placement.
#[test]
fn a_label_near_the_screen_edge_stays_on_screen() {
    let style = CursorOverlayStyle::default();
    let mut edge = frame(&style, Some("open the file"));
    edge.tip = Point {
        x: 1600.0,
        y: 770.0,
    };

    let composed = compose(&edge);
    let text = composed.text_rect.expect("a text rectangle");

    let absolute_right = composed.origin.x + text.x + text.width;
    let absolute_bottom = composed.origin.y + text.y + text.height;
    assert!(
        absolute_right <= screen().width + 1.0,
        "{absolute_right} runs off the right"
    );
    assert!(
        absolute_bottom <= screen().height + 1.0,
        "{absolute_bottom} runs off the bottom"
    );
}

/// The surface follows the pose rather than spanning the screen; painting a
/// virtual screen every frame is a frame budget rather than a rounding error.
#[test]
fn the_surface_follows_the_pose_rather_than_spanning_the_screen() {
    let style = CursorOverlayStyle::default();

    let composed = compose(&frame(&style, None));

    assert!(
        f64::from(composed.surface.width) < screen().width / 4.0,
        "a {}px surface is no longer a follower",
        composed.surface.width
    );
}

#[test]
fn a_larger_style_size_yields_a_larger_surface() {
    let small = CursorOverlayStyle::default();
    let mut large = CursorOverlayStyle::default();
    large.set_size(3.0);

    let small_surface = compose(&frame(&small, None)).surface.width;
    let large_surface = compose(&frame(&large, None)).surface.width;

    assert!(large_surface > small_surface);
}

/// The frame budget, stated as a test rather than as a hope.
///
/// The travel loop samples core's trajectory on a clock and composes one
/// surface per frame, so smoothness is bounded by what composing costs
/// against the display's frame interval. A compose that overran the interval
/// would not drop the arrival instant - core owns that - but it would make
/// the motion visibly step. Measured the way the probe corpus measures cost:
/// min of seven with the warm-up discarded.
#[test]
fn composing_the_busiest_frame_fits_well_inside_a_display_frame() {
    let style = CursorOverlayStyle::default();
    let target = Rect {
        x: 380.0,
        y: 280.0,
        width: 220.0,
        height: 44.0,
    };
    let busiest = || Frame {
        tip: Point { x: 400.0, y: 300.0 },
        style: &style,
        ripple_phase: 0.5,
        target: Some(target),
        highlight_opacity: 1.0,
        label: Some("Click the Submit button"),
        screen: screen(),
    };

    let mut samples = Vec::new();
    for run in 0..8 {
        let started = std::time::Instant::now();
        let composed = compose(&busiest());
        let elapsed = started.elapsed();
        assert!(painted_pixels(&composed) > 0, "the frame actually drew");
        if run > 0 {
            samples.push(elapsed);
        }
    }
    let best = samples.iter().min().copied().unwrap_or_default();
    let worst = samples.iter().max().copied().unwrap_or_default();
    eprintln!("busiest frame: min {best:?} max {worst:?}");

    if !budget_is_enforced() {
        return;
    }
    assert!(
        best < budget(9),
        "composing the busiest frame took {best:?} at best and {worst:?} at worst; 60Hz is the \
         refresh the schedule falls back to, and its 16.6ms has to cover this compose and the \
         layered present that follows it"
    );
}

/// What the travel actually composes: the glyph, the label that follows it,
/// and the outline of the element it is heading for. No ripple and no
/// highlight - those belong to the effect that plays after dispatch has
/// already confirmed, so they never sit inside the motion the caller waits
/// on. This is the frame whose cost decides whether the movement steps.
#[test]
fn composing_a_travel_frame_leaves_most_of_the_display_frame_unspent() {
    let style = CursorOverlayStyle::default();
    let target = Rect {
        x: 380.0,
        y: 280.0,
        width: 220.0,
        height: 44.0,
    };
    let travel = || Frame {
        tip: Point { x: 400.0, y: 300.0 },
        style: &style,
        ripple_phase: 0.0,
        target: Some(target),
        highlight_opacity: 0.0,
        label: Some("Click the Submit button"),
        screen: screen(),
    };

    let mut samples = Vec::new();
    for run in 0..8 {
        let started = std::time::Instant::now();
        let composed = compose(&travel());
        let elapsed = started.elapsed();
        assert!(painted_pixels(&composed) > 0, "the frame actually drew");
        if run > 0 {
            samples.push(elapsed);
        }
    }
    let best = samples.iter().min().copied().unwrap_or_default();
    let worst = samples.iter().max().copied().unwrap_or_default();
    eprintln!("travel frame: min {best:?} max {worst:?}");

    if !budget_is_enforced() {
        return;
    }
    assert!(
        best < budget(5),
        "composing a travel frame took {best:?} at best and {worst:?} at worst; the motion is \
         sampled once per display frame, so a compose near the interval makes it step"
    );
}

/// The highlight is the only part of a frame whose cost is set by what the
/// caller clicked rather than by the cursor: outlining a scroll container
/// spans a surface a hundred times the area of a button's. Composing is
/// sampled once per display frame, so a large target that overran the
/// interval would make every motion toward it visibly step - and the sizes
/// below are ordinary application furniture, not a contrived worst case.
///
/// Measured the way the probe corpus measures cost: min of seven with the
/// warm-up discarded, printed always and asserted only where wall-clock
/// numbers mean something.
#[test]
fn composing_frames_around_large_targets_fits_inside_a_display_frame() {
    let style = CursorOverlayStyle::default();
    let mut slowest = std::time::Duration::ZERO;
    for (width, height) in [(220.0, 44.0), (600.0, 400.0), (1200.0, 800.0)] {
        let target = Rect {
            x: 380.0,
            y: 280.0,
            width,
            height,
        };
        let around_target = || Frame {
            tip: Point {
                x: target.x + target.width / 2.0,
                y: target.y + target.height / 2.0,
            },
            style: &style,
            ripple_phase: 0.5,
            target: Some(target),
            highlight_opacity: 1.0,
            label: Some("Click the Submit button"),
            screen: screen(),
        };

        let mut samples = Vec::new();
        let mut surface = (0, 0);
        for run in 0..8 {
            let started = std::time::Instant::now();
            let composed = compose(&around_target());
            let elapsed = started.elapsed();
            assert!(painted_pixels(&composed) > 0, "the frame actually drew");
            surface = (composed.surface.width, composed.surface.height);
            if run > 0 {
                samples.push(elapsed);
            }
        }
        let best = samples.iter().min().copied().unwrap_or_default();
        let worst = samples.iter().max().copied().unwrap_or_default();
        eprintln!(
            "target {width}x{height} -> surface {}x{} -> min {best:?} max {worst:?}",
            surface.0, surface.1
        );
        slowest = slowest.max(best);
    }

    if !budget_is_enforced() {
        return;
    }
    assert!(
        slowest < budget(20),
        "the slowest large-target compose took {slowest:?} at best. This bound is deliberately \
         looser than the two above, and it is not policing a margin: outlining an element the \
         size of a window measured 13ms against a 15.6ms interval, so a bound at the interval \
         itself would fail on ordinary run-to-run spread and teach whoever met it to widen the \
         bound. What it exists to catch is the regression that produced it - a full-rectangle \
         walk over the outline's interior, which measured 339ms - and any regression of even \
         half that size still crosses this"
    );
}
