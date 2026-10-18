//! Small native, per-pixel glass material. The compositor supplies real
//! transparency; the rim models reflections, not background-image refraction.
#![allow(unsafe_op_in_unsafe_fn)]

use std::{ffi::c_void, mem::size_of, ptr};

use windows_sys::Win32::{
    Foundation::{COLORREF, HWND, POINT, SIZE},
    Graphics::Gdi::{
        AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
        CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, HBITMAP, HDC,
        HGDIOBJ, SelectObject,
    },
    UI::WindowsAndMessaging::{ULW_ALPHA, UpdateLayeredWindow},
};

pub struct Surface {
    pub dc: HDC,
    bitmap: HBITMAP,
    previous: HGDIOBJ,
    bits: *mut u32,
    pub width: i32,
    pub height: i32,
}

impl Surface {
    pub fn new(width: i32, height: i32) -> Option<Self> {
        let pixels = (width as i64).checked_mul(height as i64)?;
        if width <= 0 || height <= 0 || pixels > 4_194_304 {
            return None;
        }
        unsafe {
            let dc = CreateCompatibleDC(ptr::null_mut());
            if dc.is_null() {
                return None;
            }
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
                    biHeight: -height,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB,
                    ..std::mem::zeroed()
                },
                ..std::mem::zeroed()
            };
            let mut bits: *mut c_void = ptr::null_mut();
            let bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, ptr::null_mut(), 0);
            if bitmap.is_null() || bits.is_null() {
                if !bitmap.is_null() {
                    DeleteObject(bitmap as HGDIOBJ);
                }
                DeleteDC(dc);
                return None;
            }
            let previous = SelectObject(dc, bitmap as HGDIOBJ);
            if previous.is_null() || previous as isize == -1 {
                DeleteObject(bitmap as HGDIOBJ);
                DeleteDC(dc);
                return None;
            }
            let mut surface = Self {
                dc,
                bitmap,
                previous,
                bits: bits.cast(),
                width,
                height,
            };
            surface.pixels().fill(0);
            Some(surface)
        }
    }

    pub fn pixels(&mut self) -> &mut [u32] {
        unsafe { std::slice::from_raw_parts_mut(self.bits, (self.width * self.height) as usize) }
    }

    pub fn present(&self, hwnd: HWND) -> bool {
        let size = SIZE {
            cx: self.width,
            cy: self.height,
        };
        let origin = POINT { x: 0, y: 0 };
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        unsafe {
            UpdateLayeredWindow(
                hwnd,
                ptr::null_mut(),
                ptr::null(),
                &size,
                self.dc,
                &origin,
                0,
                &blend,
                ULW_ALPHA,
            ) != 0
        }
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.previous);
            DeleteObject(self.bitmap as HGDIOBJ);
            DeleteDC(self.dc);
        }
    }
}

// All colors are premultiplied BGRA, as required by UpdateLayeredWindow.
fn over(pixel: u32, rgb: [f32; 3], alpha: f32) -> u32 {
    let a = alpha.clamp(0.0, 1.0);
    let old_a = (pixel >> 24) as f32;
    let channel = |shift: u32, color: f32| {
        (color * 255.0 * a + ((pixel >> shift) & 255) as f32 * (1.0 - a)).round() as u32
    };
    ((a * 255.0 + old_a * (1.0 - a)).round() as u32) << 24
        | channel(16, rgb[0]) << 16
        | channel(8, rgb[1]) << 8
        | channel(0, rgb[2])
}

fn distance(x: f32, y: f32, width: f32, height: f32, scale: f32) -> (f32, f32, f32) {
    let radius = ((height - 4.0 * scale) * 0.5).max(scale);
    let px = x - width * 0.5;
    let py = y - height * 0.5;
    let qx = px.abs() - (width * 0.5 - 2.0 * scale - radius).max(0.0);
    let qy = py.abs() - (height * 0.5 - 2.0 * scale - radius).max(0.0);
    let ax = qx.max(0.0);
    let ay = qy.max(0.0);
    let length = (ax * ax + ay * ay).sqrt();
    let (nx, ny) = if length > 0.001 {
        (ax / length * px.signum(), ay / length * py.signum())
    } else if qx > qy {
        (px.signum(), 0.0)
    } else {
        (0.0, py.signum())
    };
    (length + qx.max(qy).min(0.0) - radius, nx, ny)
}

pub fn material(
    width: i32,
    height: i32,
    scale: f32,
    dark: bool,
    ambient: COLORREF,
    pointer: Option<(i32, i32)>,
) -> Vec<u32> {
    let mut pixels = vec![0; (width * height) as usize];
    let env = [
        (ambient & 255) as f32 / 255.0,
        ((ambient >> 8) & 255) as f32 / 255.0,
        ((ambient >> 16) & 255) as f32 / 255.0,
    ];
    let tint = env.map(|c| c * 0.9 + if dark { 0.065 } else { 0.1 });
    let light_x = pointer
        .map(|p| p.0 as f32 / width as f32 - 0.5)
        .unwrap_or(-0.3);
    let light_length = (light_x * light_x + 0.64).sqrt();
    let lx = light_x / light_length;
    let ly = -0.8 / light_length;
    for y in 0..height {
        for x in 0..width {
            let (d, nx, ny) = distance(
                x as f32 + 0.5,
                y as f32 + 0.5,
                width as f32,
                height as f32,
                scale,
            );
            let coverage = (0.5 - d / scale).clamp(0.0, 1.0);
            let mut pixel = 0;
            // A restrained downward shadow, rather than a large glowing halo.
            if d > 0.0 {
                let (sd, _, _) = distance(
                    x as f32 + 0.5,
                    y as f32 + 0.5 - scale,
                    width as f32,
                    height as f32,
                    scale,
                );
                if sd < 3.0 * scale {
                    pixel = over(
                        pixel,
                        [0.0; 3],
                        0.065 * (-sd.max(0.0) / (1.4 * scale)).exp(),
                    );
                }
            }
            if coverage > 0.0 {
                let edge = (-d / scale).max(0.0);
                pixel = over(pixel, tint, coverage * if dark { 0.23 } else { 0.16 });
                let reflection = (nx * lx + ny * ly).max(0.0).powi(5);
                let rim = (-edge / 0.65).exp();
                pixel = over(pixel, [1.0; 3], coverage * rim * (0.12 + 0.62 * reflection));
                let inner = (-(edge - 1.25).powi(2) / 0.55).exp();
                pixel = over(
                    pixel,
                    [0.0; 3],
                    coverage * inner * 0.095 * (1.0 - reflection),
                );
                let lens = (-(edge - 2.4).powi(2) / 1.5).exp();
                pixel = over(pixel, [1.0; 3], coverage * lens * reflection * 0.10);
                if let Some((px, py)) = pointer {
                    let dx = (x - px) as f32 / (55.0 * scale);
                    let dy = (y - py) as f32 / (18.0 * scale);
                    pixel = over(
                        pixel,
                        [1.0; 3],
                        coverage * 0.025 * (-(dx * dx + dy * dy)).exp(),
                    );
                }
            }
            pixels[(y * width + x) as usize] = pixel;
        }
    }
    pixels
}

/// Area-average RGB coverage without relying on GDI's small-font bitmap strikes.
/// Alpha is not used here: the separate coverage mask supplies it at composition.
pub fn downsample_rgb(source: &[u32], width: usize, height: usize, factor: usize) -> Vec<u32> {
    assert!(factor > 0);
    let source_width = width * factor;
    assert_eq!(source.len(), source_width * height * factor);
    let mut result = vec![0; width * height];
    let count = (factor * factor) as u32;
    for y in 0..height {
        for x in 0..width {
            let mut channels = [0_u32; 3];
            for dy in 0..factor {
                for dx in 0..factor {
                    let pixel = source[(y * factor + dy) * source_width + x * factor + dx];
                    for (channel, shift) in channels.iter_mut().zip([0, 8, 16]) {
                        *channel += (pixel >> shift) & 255;
                    }
                }
            }
            result[y * width + x] = channels
                .into_iter()
                .enumerate()
                .fold(0, |pixel, (channel, sum)| {
                    pixel | ((sum + count / 2) / count) << (channel * 8)
                });
        }
    }
    result
}

/// Color and white coverage masks are drawn separately with grayscale GDI AA.
/// This preserves opaque, legible text without opaque rectangles or color keys.
pub fn composite(material: &mut [u32], color: &[u32], mask: &[u32]) {
    assert_eq!(material.len(), color.len());
    assert_eq!(material.len(), mask.len());
    for ((background, foreground), mask) in material.iter_mut().zip(color).zip(mask) {
        let alpha = (mask & 255).max((mask >> 8) & 255).max((mask >> 16) & 255);
        let remaining = 255 - alpha;
        let channel = |shift: u32| {
            (((*foreground >> shift) & 255)
                + (((*background >> shift) & 255) * remaining + 127) / 255)
                .min(255)
        };
        *background = (alpha + ((*background >> 24) * remaining + 127) / 255) << 24
            | channel(16) << 16
            | channel(8) << 8
            | channel(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downsampling_keeps_flat_colors_and_fractional_stroke_coverage() {
        assert_eq!(downsample_rgb(&[0x123456; 36], 2, 2, 3), vec![0x123456; 4]);
        // A one-pixel-wide stroke at triple resolution has 1/3 native coverage.
        let stroke = [0xFFFFFF, 0, 0, 0xFFFFFF, 0, 0, 0xFFFFFF, 0, 0];
        assert_eq!(downsample_rgb(&stroke, 1, 1, 3), vec![0x555555]);
        assert_eq!(
            downsample_rgb(&[0, 0xFFFFFF, 0, 0xFFFFFF], 2, 2, 1),
            vec![0, 0xFFFFFF, 0, 0xFFFFFF]
        );
    }

    #[test]
    fn surfaces_release_gdi_resources_and_reject_oversized_allocations() {
        use windows_sys::Win32::System::Threading::{
            GR_GDIOBJECTS, GetCurrentProcess, GetGuiResources,
        };
        assert!(Surface::new(0, 30).is_none());
        assert!(Surface::new(i32::MAX, i32::MAX).is_none());
        drop(Surface::new(1, 1).unwrap());
        let before = unsafe { GetGuiResources(GetCurrentProcess(), GR_GDIOBJECTS) };
        for _ in 0..100 {
            let mut surface = Surface::new(220, 30).unwrap();
            surface.pixels()[0] = 0x80808080;
        }
        let after = unsafe { GetGuiResources(GetCurrentProcess(), GR_GDIOBJECTS) };
        assert!(
            after <= before + 4,
            "GDI handles leaked: {before} -> {after}"
        );
    }

    #[test]
    fn glass_is_translucent_with_clear_corners_and_valid_premultiplication() {
        for (width, height, scale) in [(220, 30, 1.0), (460, 30, 1.0), (690, 45, 1.5)] {
            for dark in [false, true] {
                let pixels = material(width, height, scale, dark, 0x787060, None);
                assert_eq!(pixels[0], 0);
                let alpha = pixels[(height / 2 * width + width / 2) as usize] >> 24;
                assert!((25..90).contains(&alpha));
                for pixel in pixels {
                    let alpha = pixel >> 24;
                    for shift in [0, 8, 16] {
                        assert!(((pixel >> shift) & 255) <= alpha);
                    }
                }
            }
        }
    }

    #[test]
    fn reflections_respond_to_pointer_and_environment() {
        let left = material(220, 30, 1.0, false, 0xe8e8e8, Some((20, 8)));
        let right = material(220, 30, 1.0, false, 0xe8e8e8, Some((200, 8)));
        let colored = material(220, 30, 1.0, false, 0x806040, None);
        assert_ne!(left, right);
        assert_ne!(left, colored);
    }

    #[test]
    fn foreground_stays_opaque_and_grayscale_edges_are_premultiplied() {
        let mut background = [0x20202020, 0x20202020, 0x20202020];
        composite(
            &mut background,
            &[0x00ffffff, 0x00808080, 0],
            &[0x00ffffff, 0x00808080, 0],
        );
        assert_eq!(background[0], 0xffffffff);
        assert_eq!(background[2], 0x20202020);
        assert_eq!(background[1] >> 24, ((background[1] >> 16) & 255));
    }
}
