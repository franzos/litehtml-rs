/// Render an HTML file in a window.
///
/// Usage: cargo run --example render --features pixbuf -- input.html [width]
use minifb::{Key, Window, WindowOptions};
use std::{env, fs, process};

use litehtml::pixbuf::PixbufContainer;
use litehtml::{Document, Position};

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} <input.html> [width]", args[0]);
        process::exit(1);
    }

    let input = &args[1];
    let width: u32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(800);
    let win_height: u32 = 600;

    let html = fs::read_to_string(input).unwrap_or_else(|e| {
        eprintln!("Cannot read {}: {}", input, e);
        process::exit(1);
    });

    // Layout the document to get its actual content height
    let mut container = PixbufContainer::new(width, win_height);
    let content_height = {
        if let Some(mut doc) = Document::from_html(&html, &mut container, None, None) {
            doc.render(width as f32);
            (doc.height().ceil() as u32).max(win_height)
        } else {
            win_height
        }
    };

    // Re-create container at full content height and render
    container.resize(width, content_height);
    if let Some(mut doc) = Document::from_html(&html, &mut container, None, None) {
        doc.render(width as f32);
        doc.draw(0, 0.0, 0.0, Some(Position {
            x: 0.0,
            y: 0.0,
            width: width as f32,
            height: content_height as f32,
        }));
    }

    // Convert RGBA to minifb's 0xRRGGBB u32 format, blended against white
    let framebuffer: Vec<u32> = container
        .pixels()
        .chunks_exact(4)
        .map(|px| {
            let (r, g, b, a) = (px[0] as u32, px[1] as u32, px[2] as u32, px[3] as u32);
            let r = (r * a + 255 * (255 - a)) / 255;
            let g = (g * a + 255 * (255 - a)) / 255;
            let b = (b * a + 255 * (255 - a)) / 255;
            (r << 16) | (g << 8) | b
        })
        .collect();

    let mut window = Window::new(
        input,
        width as usize,
        win_height as usize,
        WindowOptions {
            resize: false,
            ..WindowOptions::default()
        },
    )
    .unwrap_or_else(|e| {
        eprintln!("Cannot create window: {}", e);
        process::exit(1);
    });

    let max_scroll = content_height.saturating_sub(win_height);
    let mut scroll_y: u32 = 0;

    while window.is_open() && !window.is_key_down(Key::Escape) {
        if let Some((_, dy)) = window.get_scroll_wheel() {
            let delta = (dy * 40.0) as i32;
            scroll_y = (scroll_y as i32 - delta).clamp(0, max_scroll as i32) as u32;
        }

        if window.is_key_down(Key::Down) {
            scroll_y = (scroll_y + 20).min(max_scroll);
        }
        if window.is_key_down(Key::Up) {
            scroll_y = scroll_y.saturating_sub(20);
        }
        if window.is_key_down(Key::PageDown) {
            scroll_y = (scroll_y + win_height).min(max_scroll);
        }
        if window.is_key_down(Key::PageUp) {
            scroll_y = scroll_y.saturating_sub(win_height);
        }
        if window.is_key_down(Key::Home) {
            scroll_y = 0;
        }
        if window.is_key_down(Key::End) {
            scroll_y = max_scroll;
        }

        let row_start = scroll_y as usize * width as usize;
        let row_end = (row_start + win_height as usize * width as usize).min(framebuffer.len());
        let visible = &framebuffer[row_start..row_end];

        if visible.len() < (win_height as usize * width as usize) {
            let mut padded = visible.to_vec();
            padded.resize(win_height as usize * width as usize, 0x00FFFFFF);
            window.update_with_buffer(&padded, width as usize, win_height as usize).unwrap();
        } else {
            window.update_with_buffer(visible, width as usize, win_height as usize).unwrap();
        }
    }
}
