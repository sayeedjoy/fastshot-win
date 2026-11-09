//! Cosmetic effects. These are **not** a privacy guarantee (PRD SEC-02):
//! blurred or pixelated text can sometimes be reconstructed.

use image::RgbaImage;

use crate::geometry::PixelRect;

/// Pixelate `rect` with square blocks of `block` px, aligned to the rect's
/// top-left so the preview (which uses the same alignment) matches.
pub fn pixelate(img: &mut RgbaImage, rect: PixelRect, block: u32) {
    let block = block.max(2);
    let (x0, y0) = (rect.x as u32, rect.y as u32);
    let (x1, y1) = (x0 + rect.width, y0 + rect.height);
    let mut by = y0;
    while by < y1 {
        let bh = block.min(y1 - by);
        let mut bx = x0;
        while bx < x1 {
            let bw = block.min(x1 - bx);
            let mut sum = [0u64; 3];
            for y in by..by + bh {
                for x in bx..bx + bw {
                    let p = img.get_pixel(x, y).0;
                    sum[0] += p[0] as u64;
                    sum[1] += p[1] as u64;
                    sum[2] += p[2] as u64;
                }
            }
            let n = (bw * bh) as u64;
            let avg = [
                (sum[0] / n) as u8,
                (sum[1] / n) as u8,
                (sum[2] / n) as u8,
                255,
            ];
            for y in by..by + bh {
                for x in bx..bx + bw {
                    img.put_pixel(x, y, image::Rgba(avg));
                }
            }
            bx += bw;
        }
        by += bh;
    }
}

/// Box sizes for approximating a Gaussian of `sigma` with `n` box blurs
/// (Kovesi / "fast almost-Gaussian" method).
fn boxes_for_gauss(sigma: f32, n: usize) -> Vec<usize> {
    let n_f = n as f32;
    let w_ideal = ((12.0 * sigma * sigma / n_f) + 1.0).sqrt();
    let mut wl = w_ideal.floor() as i64;
    if wl % 2 == 0 {
        wl -= 1;
    }
    let wu = wl + 2;
    let m_ideal =
        (12.0 * sigma * sigma - n_f * (wl * wl) as f32 - 4.0 * n_f * wl as f32 - 3.0 * n_f)
            / (-4.0 * wl as f32 - 4.0);
    let m = m_ideal.round() as i64;
    (0..n as i64)
        .map(|i| {
            if i < m {
                wl.max(1) as usize
            } else {
                wu.max(1) as usize
            }
        })
        .collect()
}

fn box_blur_h(src: &[f32], dst: &mut [f32], w: usize, h: usize, r: usize) {
    let scale = 1.0 / (2 * r + 1) as f32;
    for y in 0..h {
        for c in 0..3 {
            let at = |x: isize| -> f32 {
                let xi = x.clamp(0, w as isize - 1) as usize;
                src[(y * w + xi) * 3 + c]
            };
            let mut acc: f32 = (-(r as isize)..=r as isize).map(at).sum();
            for x in 0..w {
                dst[(y * w + x) * 3 + c] = acc * scale;
                acc += at(x as isize + r as isize + 1) - at(x as isize - r as isize);
            }
        }
    }
}

fn box_blur_v(src: &[f32], dst: &mut [f32], w: usize, h: usize, r: usize) {
    let scale = 1.0 / (2 * r + 1) as f32;
    for x in 0..w {
        for c in 0..3 {
            let at = |y: isize| -> f32 {
                let yi = y.clamp(0, h as isize - 1) as usize;
                src[(yi * w + x) * 3 + c]
            };
            let mut acc: f32 = (-(r as isize)..=r as isize).map(at).sum();
            for y in 0..h {
                dst[(y * w + x) * 3 + c] = acc * scale;
                acc += at(y as isize + r as isize + 1) - at(y as isize - r as isize);
            }
        }
    }
}

/// Gaussian-like blur of `rect`. Samples a padded neighbourhood (like the SVG
/// `feGaussianBlur` preview does) and writes back only inside `rect`.
pub fn blur(img: &mut RgbaImage, rect: PixelRect, sigma: f32) {
    let pad = (sigma * 3.0).ceil() as i64;
    let (iw, ih) = (img.width() as i64, img.height() as i64);
    let px0 = (rect.x as i64 - pad).max(0);
    let py0 = (rect.y as i64 - pad).max(0);
    let px1 = (rect.right() + pad).min(iw);
    let py1 = (rect.bottom() + pad).min(ih);
    let (w, h) = ((px1 - px0) as usize, (py1 - py0) as usize);
    if w == 0 || h == 0 {
        return;
    }
    let mut a = vec![0f32; w * h * 3];
    for y in 0..h {
        for x in 0..w {
            let p = img
                .get_pixel((px0 as usize + x) as u32, (py0 as usize + y) as u32)
                .0;
            let i = (y * w + x) * 3;
            a[i] = p[0] as f32;
            a[i + 1] = p[1] as f32;
            a[i + 2] = p[2] as f32;
        }
    }
    let mut b = vec![0f32; w * h * 3];
    for size in boxes_for_gauss(sigma, 3) {
        let r = (size - 1) / 2;
        box_blur_h(&a, &mut b, w, h, r);
        box_blur_v(&b, &mut a, w, h, r);
    }
    for y in rect.y as i64..rect.bottom() {
        for x in rect.x as i64..rect.right() {
            let i = ((y - py0) as usize * w + (x - px0) as usize) * 3;
            img.put_pixel(
                x as u32,
                y as u32,
                image::Rgba([a[i] as u8, a[i + 1] as u8, a[i + 2] as u8, 255]),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checker(w: u32, h: u32) -> RgbaImage {
        RgbaImage::from_fn(w, h, |x, y| {
            if (x + y) % 2 == 0 {
                image::Rgba([255, 255, 255, 255])
            } else {
                image::Rgba([0, 0, 0, 255])
            }
        })
    }

    #[test]
    fn pixelate_makes_uniform_blocks() {
        let mut img = checker(8, 8);
        pixelate(&mut img, PixelRect::new(0, 0, 8, 8), 4);
        let first = *img.get_pixel(0, 0);
        for y in 0..4 {
            for x in 0..4 {
                assert_eq!(*img.get_pixel(x, y), first);
            }
        }
        assert!(first.0[0] > 100 && first.0[0] < 150);
    }

    #[test]
    fn pixelate_only_touches_rect() {
        let mut img = checker(8, 8);
        let before = *img.get_pixel(7, 7);
        pixelate(&mut img, PixelRect::new(0, 0, 4, 4), 4);
        assert_eq!(*img.get_pixel(7, 7), before);
    }

    #[test]
    fn blur_smooths_and_stays_in_rect() {
        let mut img = checker(32, 32);
        let outside = *img.get_pixel(0, 0);
        blur(&mut img, PixelRect::new(8, 8, 16, 16), 3.0);
        let mid = img.get_pixel(16, 16).0[0];
        assert!(
            (100..=155).contains(&mid),
            "blurred checker should be grey, got {mid}"
        );
        assert_eq!(*img.get_pixel(0, 0), outside);
    }

    #[test]
    fn box_sizes_are_odd() {
        for s in [1.0, 2.5, 8.0, 40.0] {
            for b in boxes_for_gauss(s, 3) {
                assert_eq!(b % 2, 1);
            }
        }
    }
}
