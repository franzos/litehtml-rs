/// Render an HTML file in a window with text selection support.
///
/// Usage: cargo run --example render --features pixbuf -- input.html [width] [--scale N]
///
/// Click and drag to select text. Selected text is printed on exit.
use minifb::{Key, MouseButton, MouseMode, Window, WindowOptions};
use std::{env, fs, process};

use litehtml::pixbuf::PixbufContainer;
use litehtml::selection::Selection;
use litehtml::{Document, Position};

/// Minimum drag distance (px) before selection starts.
const DRAG_THRESHOLD: f32 = 4.0;

/// Auto-scroll edge zone (px from top/bottom).
const SCROLL_EDGE: f32 = 20.0;

/// Max auto-scroll speed (px/frame).
const SCROLL_SPEED_MAX: f32 = 12.0;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} <input.html> [width] [--scale N]", args[0]);
        process::exit(1);
    }

    let input = &args[1];
    let width: u32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(800);
    let win_height: u32 = 600;

    // Parse --scale flag
    let scale: f32 = args
        .iter()
        .position(|a| a == "--scale")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(1.0);

    let html = fs::read_to_string(input).unwrap_or_else(|e| {
        eprintln!("Cannot read {}: {}", input, e);
        process::exit(1);
    });

    // Physical pixel dimensions for the window buffer
    let phys_width = ((width as f32) * scale).ceil() as u32;
    let phys_win_height = ((win_height as f32) * scale).ceil() as u32;

    // First pass: measure content height (logical)
    let mut container = PixbufContainer::new_with_scale(width, win_height, scale);
    let content_height = {
        if let Ok(mut doc) = Document::from_html(&html, &mut container, None, None) {
            let _ = doc.render(width as f32);
            (doc.height().ceil() as u32).max(win_height)
        } else {
            win_height
        }
    };

    // Second pass: render at full content height (logical)
    container.resize_with_scale(width, content_height, scale);
    if let Ok(mut doc) = Document::from_html(&html, &mut container, None, None) {
        let _ = doc.render(width as f32);
        doc.draw(
            0,
            0.0,
            0.0,
            Some(Position {
                x: 0.0,
                y: 0.0,
                width: width as f32,
                height: content_height as f32,
            }),
        );
    }

    // Save base framebuffer (premultiplied RGBA composited against white)
    // The pixmap is at physical resolution
    let base_framebuffer = premul_to_rgb(container.pixels());

    // Third pass: create document for interactive selection (layout only, no draw)
    let measure = container.text_measure_fn();
    let doc = match Document::from_html(&html, &mut container, None, None) {
        Ok(mut d) => {
            let _ = d.render(width as f32);
            d
        }
        Err(e) => {
            eprintln!("Failed to create document: {:?}", e);
            process::exit(1);
        }
    };

    let mut selection = Selection::for_document(&doc);
    let mut selection_rects: Vec<Position> = Vec::new();
    let mut mouse_was_down = false;
    let mut drag_origin: Option<(f32, f32)> = None;
    let mut drag_active = false;
    let mut last_mouse: Option<(f32, f32)> = None;

    // Window size is physical pixels (minifb displays 1:1)
    let mut window = Window::new(
        input,
        phys_width as usize,
        phys_win_height as usize,
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
        // Scroll handling (in logical units)
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

        // Mouse selection — minifb reports window-pixel coords which are physical.
        // Convert to logical for litehtml.
        let mouse_down = window.get_mouse_down(MouseButton::Left);
        if let Some((mx_phys, my_phys)) = window.get_mouse_pos(MouseMode::Clamp) {
            let mx = mx_phys / scale;
            let my = my_phys / scale;
            let doc_x = mx;
            let doc_y = my + scroll_y as f32;

            if mouse_down && !mouse_was_down {
                drag_origin = Some((mx, my));
                drag_active = false;
                selection.clear();
                selection_rects.clear();
                last_mouse = Some((mx, my));
            } else if mouse_down {
                let moved = last_mouse.map_or(true, |(lx, ly)| {
                    (mx - lx).abs() > 0.5 || (my - ly).abs() > 0.5
                });

                if moved {
                    last_mouse = Some((mx, my));

                    if !drag_active {
                        if let Some((ox, oy)) = drag_origin {
                            let dist = ((mx - ox).powi(2) + (my - oy).powi(2)).sqrt();
                            if dist >= DRAG_THRESHOLD {
                                drag_active = true;
                                let origin_doc_y = oy + scroll_y as f32;
                                selection.start_at(&doc, &measure, ox, origin_doc_y, ox, oy);
                            }
                        }
                    }

                    if drag_active {
                        selection.extend_to(&doc, &measure, doc_x, doc_y, mx, my);
                        selection_rects = selection.rectangles().to_vec();

                        if my < SCROLL_EDGE {
                            let factor = 1.0 - (my / SCROLL_EDGE).max(0.0);
                            let speed = (factor * SCROLL_SPEED_MAX).ceil() as u32;
                            scroll_y = scroll_y.saturating_sub(speed);
                        } else if my > win_height as f32 - SCROLL_EDGE {
                            let over = my - (win_height as f32 - SCROLL_EDGE);
                            let factor = (over / SCROLL_EDGE).min(1.0);
                            let speed = (factor * SCROLL_SPEED_MAX).ceil() as u32;
                            scroll_y = (scroll_y + speed).min(max_scroll);
                        }
                    }
                }
            } else {
                if drag_active {
                    drag_active = false;
                }
                drag_origin = None;
            }
        }
        mouse_was_down = mouse_down;

        // Build visible slice from base framebuffer (physical coords)
        let phys_scroll_y = ((scroll_y as f32) * scale).ceil() as u32;
        let row_start = phys_scroll_y as usize * phys_width as usize;
        let row_end = (row_start + phys_win_height as usize * phys_width as usize)
            .min(base_framebuffer.len());
        let mut visible: Vec<u32> = base_framebuffer[row_start..row_end].to_vec();

        // Overlay selection highlight (scale rects to physical)
        for rect in &selection_rects {
            let phys_rect = Position {
                x: rect.x * scale,
                y: rect.y * scale,
                width: rect.width * scale,
                height: rect.height * scale,
            };
            overlay_selection_rect(
                &mut visible,
                phys_width,
                phys_scroll_y,
                phys_win_height,
                &phys_rect,
            );
        }

        let expected = phys_win_height as usize * phys_width as usize;
        if visible.len() < expected {
            visible.resize(expected, 0x00FFFFFF);
        }

        window
            .update_with_buffer(&visible, phys_width as usize, phys_win_height as usize)
            .unwrap();
    }

    // Print selected text on exit
    if let Some(text) = selection.selected_text() {
        if !text.is_empty() {
            println!("Selected: {}", text);
        }
    }
}

/// Convert premultiplied RGBA pixels to 0xRRGGBB composited against white.
fn premul_to_rgb(pixels: &[u8]) -> Vec<u32> {
    pixels
        .chunks_exact(4)
        .map(|px| {
            let (r, g, b, a) = (px[0] as u32, px[1] as u32, px[2] as u32, px[3] as u32);
            // Premultiplied source-over against white (255):
            // out = src_premul + dst * (1 - src_alpha)
            let r = (r + (255 * (255 - a) + 127) / 255).min(255);
            let g = (g + (255 * (255 - a) + 127) / 255).min(255);
            let b = (b + (255 * (255 - a) + 127) / 255).min(255);
            (r << 16) | (g << 8) | b
        })
        .collect()
}

/// Overlay a semi-transparent blue rectangle onto the visible framebuffer.
fn overlay_selection_rect(
    buf: &mut [u32],
    buf_width: u32,
    scroll_y: u32,
    win_height: u32,
    rect: &Position,
) {
    let x0 = (rect.x.max(0.0) as u32).min(buf_width);
    let x1 = ((rect.x + rect.width).max(0.0) as u32).min(buf_width);
    let y0 = (rect.y as i32 - scroll_y as i32).max(0) as u32;
    let y1 = ((rect.y + rect.height) as i32 - scroll_y as i32).clamp(0, win_height as i32) as u32;

    for y in y0..y1 {
        for x in x0..x1 {
            let idx = (y * buf_width + x) as usize;
            if idx < buf.len() {
                let pixel = buf[idx];
                let r = (pixel >> 16) & 0xFF;
                let g = (pixel >> 8) & 0xFF;
                let b = pixel & 0xFF;
                // Blend ~30% blue highlight (100, 150, 255)
                let r = (r * 70 + 100 * 30) / 100;
                let g = (g * 70 + 150 * 30) / 100;
                let b = (b * 70 + 255 * 30) / 100;
                buf[idx] = (r.min(255) << 16) | (g.min(255) << 8) | b.min(255);
            }
        }
    }
}
