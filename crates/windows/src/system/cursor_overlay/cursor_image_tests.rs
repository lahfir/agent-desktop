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

fn write_png(path: &std::path::Path, width: u32) {
    let file = std::fs::File::create(path).expect("create png");
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, 1);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("png header");
    writer
        .write_image_data(&vec![255; width as usize * 4])
        .expect("png data");
}

#[test]
fn a_png_replaced_at_the_same_path_is_drawn_from_its_new_contents() {
    let path = std::env::temp_dir().join(format!("cursor-{}.png", std::process::id()));
    let image = CursorImage::new(
        path.to_string_lossy().into_owned(),
        Point { x: 0.0, y: 0.0 },
    )
    .expect("valid image");
    write_png(&path, 1);
    let first = decoded(&image).map(|image| image.width);

    write_png(&path, 2);
    let second = decoded(&image).map(|image| image.width);
    let _ = std::fs::remove_file(&path);

    assert_eq!((first, second), (Some(1), Some(2)));
}
