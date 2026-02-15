/// CPU-generated jersey texture with bib number for cyclist rendering.

// 5x7 bitmap font for digits 0-9 (each row is a u8, 5 bits used)
const DIGIT_GLYPHS: [[u8; 7]; 10] = [
    // 0
    [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
    // 1
    [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
    // 2
    [0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b10000, 0b11111],
    // 3
    [0b01110, 0b10001, 0b00001, 0b00110, 0b00001, 0b10001, 0b01110],
    // 4
    [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
    // 5
    [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
    // 6
    [0b01110, 0b10000, 0b11110, 0b10001, 0b10001, 0b10001, 0b01110],
    // 7
    [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
    // 8
    [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
    // 9
    [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b10001, 0b01110],
];

/// Generate a 64x64 RGBA jersey texture with bib number.
///
/// - `base_color`: jersey fill color (e.g. `[255, 191, 0]` for maillot jaune)
/// - `number`: bib number to render (supports multi-digit up to 3 digits)
/// - `number_color`: digit color (e.g. `[0, 0, 0]` for black)
///
/// Returns `(rgba_bytes, width, height)`.
pub fn generate_jersey_texture(
    base_color: [u8; 3],
    number: u32,
    number_color: [u8; 3],
) -> (Vec<u8>, u32, u32) {
    let w: u32 = 64;
    let h: u32 = 64;
    let mut pixels = vec![0u8; (w * h * 4) as usize];

    // Fill with base color
    for y in 0..h {
        for x in 0..w {
            let idx = ((y * w + x) * 4) as usize;
            pixels[idx] = base_color[0];
            pixels[idx + 1] = base_color[1];
            pixels[idx + 2] = base_color[2];
            pixels[idx + 3] = 255;
        }
    }

    // Extract digits
    let digits: Vec<u32> = if number == 0 {
        vec![0]
    } else {
        let mut ds = Vec::new();
        let mut n = number;
        while n > 0 {
            ds.push(n % 10);
            n /= 10;
        }
        ds.reverse();
        ds
    };

    // Each digit is 5px wide with 1px gap, scaled 2x
    let scale = 2u32;
    let digit_w = 5 * scale;
    let gap = 1 * scale;
    let total_w = digits.len() as u32 * digit_w + (digits.len() as u32).saturating_sub(1) * gap;
    let digit_h = 7 * scale;

    // White dossard patch behind the number (TdF style)
    let patch_pad_x = 3u32;
    let patch_pad_y = 3u32;
    let patch_w = total_w + patch_pad_x * 2;
    let patch_h = digit_h + patch_pad_y * 2;
    let patch_x = (w - patch_w) / 2;
    let patch_y = (h - patch_h) / 2;

    // Draw rounded white patch
    for py in patch_y..patch_y + patch_h {
        for px in patch_x..patch_x + patch_w {
            // Simple rounded corners: skip corner pixels
            let dx = if px < patch_x + 2 {
                patch_x + 2 - px
            } else if px >= patch_x + patch_w - 2 {
                px - (patch_x + patch_w - 3)
            } else {
                0
            };
            let dy = if py < patch_y + 2 {
                patch_y + 2 - py
            } else if py >= patch_y + patch_h - 2 {
                py - (patch_y + patch_h - 3)
            } else {
                0
            };
            if dx + dy > 2 {
                continue;
            }

            let idx = ((py * w + px) * 4) as usize;
            if idx + 3 < pixels.len() {
                pixels[idx] = 255;
                pixels[idx + 1] = 255;
                pixels[idx + 2] = 255;
                pixels[idx + 3] = 255;
            }
        }
    }

    // Draw digits (scaled 2x)
    let start_x = (w - total_w) / 2;
    let start_y = (h - digit_h) / 2;
    for (i, &d) in digits.iter().enumerate() {
        let dx = start_x + i as u32 * (digit_w + gap);
        // Draw each glyph pixel as a scale x scale block
        let glyph = &DIGIT_GLYPHS[d as usize % 10];
        for row in 0..7u32 {
            let bits = glyph[row as usize];
            for col in 0..5u32 {
                if bits & (1 << (4 - col)) != 0 {
                    for sy in 0..scale {
                        for sx in 0..scale {
                            let px = dx + col * scale + sx;
                            let py = start_y + row * scale + sy;
                            if px < w && py < h {
                                let idx = ((py * w + px) * 4) as usize;
                                pixels[idx] = number_color[0];
                                pixels[idx + 1] = number_color[1];
                                pixels[idx + 2] = number_color[2];
                                pixels[idx + 3] = 255;
                            }
                        }
                    }
                }
            }
        }
    }

    (pixels, w, h)
}
