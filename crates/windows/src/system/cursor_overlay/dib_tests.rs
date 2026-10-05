use super::Dib;

/// The surface has to be exactly the size that was asked for, because both
/// callers index it by their own width and would read past the end - or paint
/// a shifted image - if the stride disagreed.
#[test]
fn the_surface_holds_exactly_the_pixels_that_were_asked_for() {
    let mut dib = Dib::create(8, 5).expect("a small surface can be made on a desktop session");

    assert_eq!(dib.pixels().len(), 40);
}

/// A refused surface must not be a partially-built one. There is no way to
/// observe a leaked DC directly from here, so what is pinned is the contract
/// the callers rely on: a refusal is `None`, not a `Dib` that is unusable.
#[test]
fn a_surface_with_no_area_is_refused_rather_than_half_made() {
    assert!(Dib::create(0, 10).is_none());
    assert!(Dib::create(10, 0).is_none());
    assert!(Dib::create(-4, 10).is_none());
}

/// The storage is the bitmap's own, so a write through the slice is what the
/// drawing calls will see. If this were a copy, the label pass would draw its
/// glyphs into a buffer nobody reads and the card would come out blank.
#[test]
fn writes_through_the_slice_reach_the_bitmaps_own_storage() {
    let mut dib = Dib::create(4, 4).expect("a small surface can be made on a desktop session");

    dib.pixels()[7] = 0x00AB_CDEF;

    assert_eq!(
        dib.pixels()[7],
        0x00AB_CDEF,
        "the slice must alias the bitmap rather than copy out of it"
    );
}
