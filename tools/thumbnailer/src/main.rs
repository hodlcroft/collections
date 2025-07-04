use clap::Parser;
use image::{DynamicImage, GenericImageView};
use std::path::PathBuf;
use webp::Encoder;

/// Resize and optimize a collection thumbnail for Hodlcroft.
#[derive(Parser, Debug)]
struct Args {
    /// Path to input PNG thumbnail
    #[arg(short, long)]
    input: PathBuf,

    /// Resize max dimension (default: 512)
    #[arg(long, default_value = "512")]
    max_dim: u32,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let input_path = &args.input;
    let mut output_path = input_path.clone();
    output_path.set_extension("webp");

    let img = image::open(input_path)?;
    let (width, height) = img.dimensions();

    // Only resize if necessary
    let resized = if width > args.max_dim || height > args.max_dim {
        resize_cover(img, args.max_dim, args.max_dim)
    } else {
        img
    };

    let resized_rgb = resized.to_rgb8();
    let encoder = Encoder::from_rgb(&resized_rgb, resized_rgb.width(), resized_rgb.height());
    let webp_data = encoder.encode(90.0); // 90% quality

    std::fs::write(&output_path, &*webp_data)?;
    println!("✅ Wrote optimized thumbnail to {}", output_path.display());

    Ok(())
}

fn resize_cover(img: DynamicImage, target_width: u32, target_height: u32) -> DynamicImage {
    let (width, height) = img.dimensions();
    let src_aspect = width as f32 / height as f32;
    let dst_aspect = target_width as f32 / target_height as f32;

    let (crop_w, crop_h) = if src_aspect > dst_aspect {
        // Image is wider than target — crop width
        let crop_w = (height as f32 * dst_aspect).round() as u32;
        (crop_w, height)
    } else {
        // Image is taller than target — crop height
        let crop_h = (width as f32 / dst_aspect).round() as u32;
        (width, crop_h)
    };

    let x_offset = (width - crop_w) / 2;
    let y_offset = (height - crop_h) / 2;

    let cropped = img.crop_imm(x_offset, y_offset, crop_w, crop_h);
    cropped.resize_exact(
        target_width,
        target_height,
        image::imageops::FilterType::Lanczos3,
    )
}
