//! Rasterizes the text console's 80x25 character grid into the virtio-gpu
//! framebuffer, replacing the legacy `0xb8000` MMIO backing (see the
//! software-console plan: virtio-gpu only works when it's the sole graphics
//! device, which means no legacy VGA text mode is available at all).

use core::sync::atomic::Ordering;
use noto_sans_mono_bitmap::{get_raster, FontWeight, RasterHeight};

use crate::device::gpu::{self, CHAR_HEIGHT, CHAR_WIDTH, GFX_HEIGHT, GFX_WIDTH};
use super::{BUFFER_HEIGHT, BUFFER_WIDTH, CONSOLE_DIRTY, WRITER};

// Classic 16-color VGA/CGA palette, as RGB triples, indexed by `Color as u8`.
const PALETTE: [(u8, u8, u8); 16] = [
    (0x00, 0x00, 0x00), // Black
    (0x00, 0x00, 0xAA), // Blue
    (0x00, 0xAA, 0x00), // Green
    (0x00, 0xAA, 0xAA), // Cyan
    (0xAA, 0x00, 0x00), // Red
    (0xAA, 0x00, 0xAA), // Magenta
    (0xAA, 0x55, 0x00), // Brown
    (0xAA, 0xAA, 0xAA), // LightGray
    (0x55, 0x55, 0x55), // DarkGray
    (0x55, 0x55, 0xFF), // LightBlue
    (0x55, 0xFF, 0x55), // LightGreen
    (0x55, 0xFF, 0xFF), // LightCyan
    (0xFF, 0x55, 0x55), // LightRed
    (0xFF, 0x55, 0xFF), // Pink
    (0xFF, 0xFF, 0x55), // Yellow
    (0xFF, 0xFF, 0xFF), // White
];

fn color_rgb(index: u8) -> (u8, u8, u8) {
    PALETTE[(index & 0x0f) as usize]
}

/// Reverse of the special cases in `super::unicode_to_cp437`; plain ASCII
/// (0x20-0x7e) needs no lookup since the byte value already equals the
/// codepoint. Anything else not covered here (unmapped high bytes) falls
/// back to '?'.
fn cp437_to_char(byte: u8) -> char {
    match byte {
        0x20..=0x7e => byte as char,
        0x86 => 'å', 0x8F => 'Å', 0x84 => 'ä', 0x8E => 'Ä',
        0x94 => 'ö', 0x99 => 'Ö', 0x82 => 'é', 0x8A => 'è',
        0x88 => 'ê', 0x89 => 'ë', 0x85 => 'à', 0x83 => 'â',
        0x97 => 'ù', 0x96 => 'û', 0x81 => 'ü', 0x9A => 'Ü',
        0xA4 => 'ñ', 0x87 => 'ç', 0x15 => '§', 0xF8 => '°',
        0x9C => '£', 0x9D => '¥', 0xEE => '€', 0xAB => '½',
        0xAC => '¼',
        _ => '?',
    }
}

fn blend(bg: (u8, u8, u8), fg: (u8, u8, u8), coverage: u8) -> (u8, u8, u8) {
    let t = coverage as u32;
    let lerp = |b: u8, f: u8| (((b as u32) * (255 - t) + (f as u32) * t) / 255) as u8;
    (lerp(bg.0, fg.0), lerp(bg.1, fg.1), lerp(bg.2, fg.2))
}

/// Rasterizes the whole character grid into `frame` (a BGRA8888 buffer sized
/// exactly `GFX_WIDTH * GFX_HEIGHT * 4`).
fn rasterize_into(frame: &mut [u8]) {
    use x86_64::instructions::interrupts;

    interrupts::without_interrupts(|| {
        let writer = WRITER.lock();
        for row in 0..BUFFER_HEIGHT {
            for col in 0..BUFFER_WIDTH {
                let cell = writer.read_at(row, col);
                let byte = cell.color_code.to_byte();
                let fg = color_rgb(byte & 0x0f);
                let bg = color_rgb((byte >> 4) & 0x0f);
                let c = cp437_to_char(cell.ascii_character);
                let glyph = get_raster(c, FontWeight::Regular, RasterHeight::Size16)
                    .unwrap_or_else(|| get_raster(' ', FontWeight::Regular, RasterHeight::Size16).unwrap());

                let base_x = col * CHAR_WIDTH as usize;
                let base_y = row * CHAR_HEIGHT as usize;
                for (gy, line) in glyph.raster().iter().enumerate() {
                    for (gx, &coverage) in line.iter().enumerate() {
                        let (r, g, b) = blend(bg, fg, coverage);
                        let idx = ((base_y + gy) * GFX_WIDTH as usize + (base_x + gx)) * 4;
                        frame[idx] = b;
                        frame[idx + 1] = g;
                        frame[idx + 2] = r;
                        frame[idx + 3] = 0xff;
                    }
                }
            }
        }
    });
}

/// Redraw loop: paced by `ticks()` (~30Hz at the reprogrammed 100Hz PIT
/// rate), only actually rasterizing + presenting when the console has
/// changed since the last frame.
pub async fn present_loop() {
    let mut frame = alloc::vec![0u8; (GFX_WIDTH * GFX_HEIGHT * 4) as usize];
    let mut last_tick = 0u64;
    loop {
        crate::task::yield_now().await;
        let now = crate::interrupts::ticks();
        if now < last_tick + 3 {
            continue;
        }
        last_tick = now;

        if !CONSOLE_DIRTY.swap(false, Ordering::AcqRel) {
            continue;
        }
        rasterize_into(&mut frame);
        gpu::present(&frame);
    }
}
