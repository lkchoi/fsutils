use image::DynamicImage;

const K1: f64 = 0.01;
const K2: f64 = 0.03;
const L: f64 = 255.0;
const C1: f64 = (K1 * L) * (K1 * L);
const C2: f64 = (K2 * L) * (K2 * L);
const WINDOW: usize = 8;

pub fn compute_ssim(img_a: &DynamicImage, img_b: &DynamicImage) -> f64 {
    let gray_a = img_a.to_luma8();
    let gray_b = img_b.to_luma8();

    let (w_a, h_a) = gray_a.dimensions();
    let (w_b, h_b) = gray_b.dimensions();

    let width = w_a.min(w_b) as usize;
    let height = h_a.min(h_b) as usize;

    // Resize if dimensions don't match
    let buf_a = if w_a as usize == width && h_a as usize == height {
        pixels_to_f64(&gray_a, width, height)
    } else {
        let resized = image::imageops::resize(
            &gray_a,
            width as u32,
            height as u32,
            image::imageops::FilterType::Triangle,
        );
        pixels_to_f64(&resized, width, height)
    };

    let buf_b = if w_b as usize == width && h_b as usize == height {
        pixels_to_f64(&gray_b, width, height)
    } else {
        let resized = image::imageops::resize(
            &gray_b,
            width as u32,
            height as u32,
            image::imageops::FilterType::Triangle,
        );
        pixels_to_f64(&resized, width, height)
    };

    if width < WINDOW || height < WINDOW {
        return 0.0;
    }

    let mut total = 0.0;
    let mut count = 0u64;

    let steps_x = (width - WINDOW) / WINDOW + 1;
    let steps_y = (height - WINDOW) / WINDOW + 1;

    for wy in 0..steps_y {
        for wx in 0..steps_x {
            let x0 = wx * WINDOW;
            let y0 = wy * WINDOW;

            let mut sum_a = 0.0;
            let mut sum_b = 0.0;
            let mut sum_a2 = 0.0;
            let mut sum_b2 = 0.0;
            let mut sum_ab = 0.0;

            for dy in 0..WINDOW {
                for dx in 0..WINDOW {
                    let idx = (y0 + dy) * width + (x0 + dx);
                    let a = buf_a[idx];
                    let b = buf_b[idx];
                    sum_a += a;
                    sum_b += b;
                    sum_a2 += a * a;
                    sum_b2 += b * b;
                    sum_ab += a * b;
                }
            }

            let n = (WINDOW * WINDOW) as f64;
            let mu_a = sum_a / n;
            let mu_b = sum_b / n;
            let sigma_a2 = sum_a2 / n - mu_a * mu_a;
            let sigma_b2 = sum_b2 / n - mu_b * mu_b;
            let sigma_ab = sum_ab / n - mu_a * mu_b;

            let numerator = (2.0 * mu_a * mu_b + C1) * (2.0 * sigma_ab + C2);
            let denominator =
                (mu_a * mu_a + mu_b * mu_b + C1) * (sigma_a2 + sigma_b2 + C2);

            total += numerator / denominator;
            count += 1;
        }
    }

    if count == 0 {
        0.0
    } else {
        total / count as f64
    }
}

fn pixels_to_f64<P>(img: &image::ImageBuffer<P, Vec<u8>>, width: usize, height: usize) -> Vec<f64>
where
    P: image::Pixel<Subpixel = u8>,
{
    let mut buf = Vec::with_capacity(width * height);
    for y in 0..height {
        for x in 0..width {
            let pixel = img.get_pixel(x as u32, y as u32);
            buf.push(pixel.channels()[0] as f64);
        }
    }
    buf
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{GrayImage, Luma};

    #[test]
    fn identical_images_score_one() {
        let img = GrayImage::from_fn(64, 64, |x, y| Luma([(x.wrapping_mul(y) % 256) as u8]));
        let dyn_img = DynamicImage::ImageLuma8(img);
        let score = compute_ssim(&dyn_img, &dyn_img);
        assert!(
            (score - 1.0).abs() < 1e-10,
            "Expected 1.0, got {}",
            score
        );
    }

    #[test]
    fn black_vs_white_scores_low() {
        let black = DynamicImage::ImageLuma8(GrayImage::from_fn(64, 64, |_, _| Luma([0])));
        let white = DynamicImage::ImageLuma8(GrayImage::from_fn(64, 64, |_, _| Luma([255])));
        let score = compute_ssim(&black, &white);
        assert!(score < 0.1, "Expected low score, got {}", score);
    }

    #[test]
    fn similar_images_score_high() {
        let img_a = GrayImage::from_fn(64, 64, |x, y| Luma([(x.wrapping_mul(y) % 256) as u8]));
        let img_b = GrayImage::from_fn(64, 64, |x, y| {
            Luma([((x.wrapping_mul(y) % 256) as u8).saturating_add(5)])
        });
        let score = compute_ssim(
            &DynamicImage::ImageLuma8(img_a),
            &DynamicImage::ImageLuma8(img_b),
        );
        assert!(score > 0.9, "Expected high score, got {}", score);
    }
}
