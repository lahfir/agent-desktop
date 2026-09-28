/// What an observation saw of a Chromium renderer's web content.
///
/// `Populated` is the evidence that matters to activation: a web area that
/// already carries child elements has its accessibility tree built, whatever
/// the application answers when asked whether accessibility is enabled.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum WebSurface {
    #[default]
    Absent,
    Empty,
    Populated,
}

impl WebSurface {
    pub(crate) fn observed(self) -> bool {
        self != Self::Absent
    }

    pub(crate) fn with_web_area(self, child_count: usize) -> Self {
        if self == Self::Populated || child_count > 0 {
            Self::Populated
        } else {
            Self::Empty
        }
    }
}

#[cfg(test)]
mod tests {
    use super::WebSurface;

    #[test]
    fn a_web_area_with_children_is_populated_and_stays_so() {
        let surface = WebSurface::Absent.with_web_area(3);
        assert_eq!(surface, WebSurface::Populated);
        assert_eq!(surface.with_web_area(0), WebSurface::Populated);
    }

    #[test]
    fn a_childless_web_area_is_observed_but_empty() {
        let surface = WebSurface::Absent.with_web_area(0);
        assert_eq!(surface, WebSurface::Empty);
        assert!(surface.observed());
        assert!(!WebSurface::Absent.observed());
    }
}
