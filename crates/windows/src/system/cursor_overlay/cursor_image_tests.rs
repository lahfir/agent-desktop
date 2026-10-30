use super::*;

#[test]
fn an_opaque_sample_keeps_its_colour_and_a_clear_one_vanishes() {
    assert_eq!(premultiplied(&[255, 174, 0, 255]), 0xFF_FF_AE_00);
    assert_eq!(premultiplied(&[255, 255, 255, 0]), 0);
}

#[test]
fn a_half_transparent_sample_is_premultiplied() {
    assert_eq!(premultiplied(&[255, 0, 0, 128]), 0x80_80_00_00);
}

#[test]
fn the_image_hangs_from_its_hotspot_at_the_style_size() {
    let image = CursorImage::new(
        if cfg!(windows) {
            "C:\\cursor.png"
        } else {
            "/cursor.png"
        }
        .into(),
        Point { x: 18.0, y: 16.0 },
    )
    .expect("valid image");
    let decoded = DecodedImage {
        width: 200,
        height: 100,
        pixels: vec![0; 20_000],
    };
    let rect = image_rect(&Point { x: 100.0, y: 50.0 }, &image, &decoded, 0.5);
    assert_eq!(
        (rect.x, rect.y, rect.width, rect.height),
        (91.0, 42.0, 100.0, 50.0)
    );
}

#[test]
fn compositing_over_a_transparent_surface_copies_the_source() {
    let mut surface = Surface::try_transparent(2, 1).expect("surface");
    composite(&mut surface, 1, 0, 0x80_40_20_10);
    assert_eq!(surface.pixels, vec![0, 0x80_40_20_10]);
}
