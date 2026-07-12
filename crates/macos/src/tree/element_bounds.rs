use agent_desktop_core::Rect;

use super::AXElement;

#[cfg(target_os = "macos")]
pub(crate) fn rect_from_parts(
    point: core_graphics::geometry::CGPoint,
    size: core_graphics::geometry::CGSize,
) -> Option<Rect> {
    if !point.x.is_finite()
        || !point.y.is_finite()
        || !size.width.is_finite()
        || !size.height.is_finite()
    {
        return None;
    }
    Some(Rect {
        x: point.x,
        y: point.y,
        width: size.width,
        height: size.height,
    })
}

#[cfg(target_os = "macos")]
pub(crate) fn read_bounds_with_deadline(
    element: &AXElement,
    deadline: std::time::Instant,
) -> Result<Option<Rect>, agent_desktop_core::AdapterError> {
    let point = super::resolve_ax_read::read_point(element, "AXPosition", deadline)?;
    let size = super::resolve_ax_read::read_size(element, "AXSize", deadline)?;
    Ok(point
        .zip(size)
        .and_then(|(point, size)| rect_from_parts(point, size)))
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn read_bounds_with_deadline(
    _element: &AXElement,
    _deadline: std::time::Instant,
) -> Result<Option<Rect>, agent_desktop_core::AdapterError> {
    Ok(None)
}
