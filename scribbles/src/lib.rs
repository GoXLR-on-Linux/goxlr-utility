use anyhow::{bail, Result};
use image::imageops::{dither, overlay, BiLevel, FilterType};
use image::{ColorType, DynamicImage, GenericImage, GenericImageView, GrayImage, Luma, Rgba};
use imageproc::drawing::{draw_text_mut, text_size};
use log::warn;
use rusttype::{Font, Scale};
use std::borrow::BorrowMut;
use std::path::PathBuf;

static FONT: &[u8] = include_bytes!("../fonts/Play-Bold.ttf");

pub fn get_scribble(
    path: Option<PathBuf>,
    bottom_text: Option<String>,
    top_right: Option<String>,
    invert: bool,
) -> [u8; 1024] {
    let mut processed_image = None;
    let mut bottom_image = None;
    let mut top_right_image = None;

    if let Some(path) = path {
        if let Ok(image) = load_grayscale_image(path) {
            processed_image = Some(image);
        }
    }

    if let Some(text) = bottom_text {
        if let Ok(image) = create_text_image(&text) {
            bottom_image = Some(image);
        }
    }

    if let Some(text) = top_right {
        if let Ok(image) = create_text_image(&text) {
            top_right_image = Some(image);
        }
    }

    let image = create_final_image(processed_image, bottom_image, top_right_image);

    if let Ok(result) = to_goxlr(image, invert) {
        result
    } else {
        [0; 1024]
    }
}

fn load_grayscale_image(path: PathBuf) -> Result<DynamicImage> {
    if !path.exists() {
        warn!("Unable to Load {}, file not found", path.to_string_lossy());
        bail!("File not Found")
    }

    let img = image::open(path)?;
    let mut img = img.grayscale();

    if img.color() == ColorType::La8 || img.color() == ColorType::L16 {
        // Ensure any fully transparent pixels are white..
        for (x, y, pixel) in img.clone().pixels() {
            if pixel[3] == 0 {
                img.put_pixel(x, y, Rgba::from([255, 255, 255, 255]));
            }
        }
    }

    Ok(img)
}

fn create_text_image(text: &str) -> Result<DynamicImage> {
    let draw_font = Font::try_from_bytes(FONT).ok_or(std::fmt::Error)?;

    let scale = Scale {
        x: 23_f32,
        y: 19_f32,
    };

    // Calculate the draw width..
    let (width, _height) = text_size(scale, &draw_font, text);
    let draw_width = if width < 128 { width } else { 128 };

    let mut image = DynamicImage::new_rgb8(draw_width as u32, 19);
    image
        .clone()
        .pixels()
        .for_each(|f| image.put_pixel(f.0, f.1, Rgba::from([255, 255, 255, 255])));

    draw_text_mut(
        &mut image,
        Rgba::from([0, 0, 0, 0]),
        0,
        0,
        scale,
        &draw_font,
        text,
    );

    Ok(image)
}

fn create_final_image(
    mut icon: Option<DynamicImage>,
    text: Option<DynamicImage>,
    number: Option<DynamicImage>,
) -> GrayImage {
    // Ok, firstly, create an image and make it completely white..
    let mut image = DynamicImage::new_rgb8(128, 64);
    image
        .clone()
        .pixels()
        .for_each(|(x, y, _pixel)| image.put_pixel(x, y, Rgba::from([255, 255, 255, 255])));

    // Ok, now we need to position and draw the specific components onto it..
    if let Some(ref mut icon) = icon {
        // We have an icon, we need to resize and position based on the existance of text..
        let (w, h) = if text.is_some() { (80, 41) } else { (120, 60) };

        // Before we resize it, we wanna stretch it by about 20% to offset the differences in pixel sizes on the GoXLR..
        *icon = icon.resize_exact(
            (icon.width() as f32 * 1.20) as u32,
            icon.height(),
            FilterType::Nearest,
        );

        // Resize the icon down to the calculated level..
        *icon = icon.resize(w, h, FilterType::Gaussian);

        // Find the middle..
        let x = (image.width() - icon.width()) / 2;
        let y = ((h - icon.height()) / 2) + 3;

        // Draw onto the main image.
        overlay(&mut image, icon, x as i64, y as i64);
    }

    if let Some(text) = text {
        let position_x = (image.width() - text.width()) / 2;
        let position_y = if icon.is_some() {
            image.height() - text.height()
        } else {
            (image.height() - text.height()) / 2
        };

        // Overlay it onto the final image..
        overlay(&mut image, &text, position_x as i64, position_y as i64);
    }

    if let Some(number) = number {
        // Shove this in the top left corner with a safety buffer..
        overlay(&mut image, &number, 5, 3);
    }

    let mut final_image = image.to_luma8();
    dither(final_image.borrow_mut(), &BiLevel);

    final_image
}

fn to_goxlr(img: GrayImage, invert: bool) -> Result<[u8; 1024]> {
    let base = if invert { 0 } else { 255 };
    assert_eq!(img.width(), 128);
    assert_eq!(img.height(), 64);

    let mut bytes: [u8; 1024] = [base; 1024];
    let white = Luma::from([255_u8]);

    for x in 0..img.width() - 1 {
        for y in 0..img.height() - 1 {
            if img.get_pixel(x, y) != &white {
                let byte = ((128 * (y / 8)) + x) as usize;
                let bit = y % 8;

                // Grab the Byte, update the bit..
                bytes[byte] ^= 1 << bit;
            }
        }
    }
    Ok(bytes)
}
