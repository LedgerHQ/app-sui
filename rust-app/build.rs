use image::{ImageFormat, ImageReader, Pixel};
use std::env;

fn main() {
    println!("cargo:rerun-if-changed=script.ld");
    println!("cargo:rerun-if-changed=sui-small.gif");
    println!("cargo:rerun-if-changed=mask_14x14.gif");

    if let Ok(path) = env::var("NEWLIB_LIB_PATH") {
        println!("cargo:rustc-link-search={path}");
    }

    let path = std::path::PathBuf::from("./");
    let reader = ImageReader::open(path.join("sui-small.gif")).unwrap();
    let img = reader.decode().unwrap();
    let mut gray = img.into_luma8();

    // Apply mask
    let mask = ImageReader::open(path.join("mask_14x14.gif"))
        .unwrap()
        .decode()
        .unwrap()
        .into_luma8();

    for (x, y, mask_pixel) in mask.enumerate_pixels() {
        let mask_value = mask_pixel[0];
        let mut gray_pixel = *gray.get_pixel(x, y);
        if mask_value == 0 {
            gray_pixel = image::Luma([0]);
        } else {
            gray_pixel.invert();
        }
        gray.put_pixel(x, y, gray_pixel);
    }

    let glyph_path = std::path::PathBuf::from("./");
    gray.save_with_format(glyph_path.join("home_nano_nbgl.png"), ImageFormat::Png)
        .unwrap();
}
