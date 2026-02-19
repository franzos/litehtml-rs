/// Fetch and render a web page by URL in a window with scrolling.
///
/// Usage: cargo run --example browse --features pixbuf -p litehtml -- <url> [width] [--height N] [--scale N] [--fullscreen]
use std::cell::RefCell;
use std::collections::HashMap;
use std::time::Instant;
use std::{env, process};

use minifb::{Key, Window, WindowOptions};
use url::Url;

use litehtml::pixbuf::PixbufContainer;
use litehtml::{
    BackgroundLayer, BorderRadiuses, Borders, Color, ConicGradient, DocumentContainer,
    FontDescription, FontMetrics, LinearGradient, ListMarker, MediaFeatures, Position,
    RadialGradient, Size, TextTransform,
};

const USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64; rv:122.0) Gecko/20100101 Firefox/122.0";

struct BrowseContainer {
    inner: PixbufContainer,
    base_url: Url,
    agent: ureq::Agent,
    css_cache: RefCell<HashMap<String, String>>,
    /// Maps raw image src → baseurl passed by litehtml, so fetch_images
    /// can resolve relative URLs against the correct context (stylesheet
    /// URL, not the page URL).
    image_baseurls: RefCell<HashMap<String, String>>,
}

impl BrowseContainer {
    fn new(base_url: Url, width: u32, height: u32, scale: f32) -> Self {
        Self {
            inner: PixbufContainer::new_with_scale(width, height, scale),
            base_url,
            agent: ureq::Agent::config_builder()
                .timeout_connect(Some(std::time::Duration::from_secs(10)))
                .timeout_recv_body(Some(std::time::Duration::from_secs(30)))
                .user_agent(USER_AGENT)
                .build()
                .new_agent(),
            css_cache: RefCell::new(HashMap::new()),
            image_baseurls: RefCell::new(HashMap::new()),
        }
    }

    /// Resolve a URL against a given base, falling back to self.base_url.
    fn resolve_against(&self, href: &str, baseurl: &str) -> Option<Url> {
        // Already absolute
        if let Ok(u) = Url::parse(href) {
            return Some(u);
        }
        // Resolve against the provided base context (e.g. stylesheet URL)
        if !baseurl.is_empty() {
            if let Ok(base) = Url::parse(baseurl) {
                if let Ok(u) = base.join(href) {
                    return Some(u);
                }
            }
        }
        // Fall back to page base URL
        self.base_url.join(href).ok()
    }

    fn fetch_url(&self, url: &Url) -> Option<Vec<u8>> {
        let resp = self.agent.get(url.as_str()).call().ok()?;
        resp.into_body().read_to_vec().ok()
    }
}

// Delegate everything to inner, override import_css, set_base_url, load_image
impl DocumentContainer for BrowseContainer {
    fn create_font(&mut self, descr: &FontDescription) -> (usize, FontMetrics) {
        self.inner.create_font(descr)
    }
    fn delete_font(&mut self, font: usize) {
        self.inner.delete_font(font);
    }
    fn text_width(&self, text: &str, font: usize) -> f32 {
        self.inner.text_width(text, font)
    }
    fn draw_text(&mut self, hdc: usize, text: &str, font: usize, color: Color, pos: Position) {
        self.inner.draw_text(hdc, text, font, color, pos);
    }
    fn draw_list_marker(&mut self, hdc: usize, marker: &ListMarker) {
        self.inner.draw_list_marker(hdc, marker);
    }
    fn load_image(&mut self, src: &str, baseurl: &str, redraw_on_ready: bool) {
        // Store the baseurl context so fetch_images can resolve correctly
        if !baseurl.is_empty() {
            self.image_baseurls
                .borrow_mut()
                .insert(src.to_string(), baseurl.to_string());
        }
        self.inner.load_image(src, baseurl, redraw_on_ready);
    }
    fn get_image_size(&self, src: &str, baseurl: &str) -> Size {
        self.inner.get_image_size(src, baseurl)
    }
    fn draw_image(&mut self, hdc: usize, layer: &BackgroundLayer, url: &str, base_url: &str) {
        self.inner.draw_image(hdc, layer, url, base_url);
    }
    fn draw_solid_fill(&mut self, hdc: usize, layer: &BackgroundLayer, color: Color) {
        self.inner.draw_solid_fill(hdc, layer, color);
    }
    fn draw_linear_gradient(
        &mut self,
        hdc: usize,
        layer: &BackgroundLayer,
        gradient: &LinearGradient,
    ) {
        self.inner.draw_linear_gradient(hdc, layer, gradient);
    }
    fn draw_radial_gradient(
        &mut self,
        hdc: usize,
        layer: &BackgroundLayer,
        gradient: &RadialGradient,
    ) {
        self.inner.draw_radial_gradient(hdc, layer, gradient);
    }
    fn draw_conic_gradient(
        &mut self,
        hdc: usize,
        layer: &BackgroundLayer,
        gradient: &ConicGradient,
    ) {
        self.inner.draw_conic_gradient(hdc, layer, gradient);
    }
    fn draw_borders(&mut self, hdc: usize, borders: &Borders, draw_pos: Position, root: bool) {
        self.inner.draw_borders(hdc, borders, draw_pos, root);
    }
    fn set_caption(&mut self, caption: &str) {
        self.inner.set_caption(caption);
    }
    fn set_base_url(&mut self, base_url: &str) {
        if let Ok(u) = Url::parse(base_url) {
            self.base_url = u;
        } else if let Ok(u) = self.base_url.join(base_url) {
            self.base_url = u;
        }
        self.inner.set_base_url(base_url);
    }
    fn on_anchor_click(&mut self, url: &str) {
        self.inner.on_anchor_click(url);
    }
    fn set_cursor(&mut self, cursor: &str) {
        self.inner.set_cursor(cursor);
    }
    fn transform_text(&self, text: &str, tt: TextTransform) -> String {
        self.inner.transform_text(text, tt)
    }
    fn import_css(&self, url: &str, baseurl: &str) -> (String, Option<String>) {
        // Resolve against the baseurl parameter (stylesheet context), not just page URL
        let resolved = match self.resolve_against(url, baseurl) {
            Some(u) => u,
            None => return (String::new(), None),
        };
        let key = resolved.to_string();
        if let Some(cached) = self.css_cache.borrow().get(&key) {
            return (cached.clone(), Some(key));
        }
        eprintln!("  CSS: {}", resolved);
        let css = match self.fetch_url(&resolved) {
            Some(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            None => return (String::new(), None),
        };
        self.css_cache.borrow_mut().insert(key.clone(), css.clone());
        // Return the resolved URL so litehtml uses it as base for url() refs in this CSS
        (css, Some(key))
    }
    fn set_clip(&mut self, pos: Position, radius: BorderRadiuses) {
        self.inner.set_clip(pos, radius);
    }
    fn del_clip(&mut self) {
        self.inner.del_clip();
    }
    fn get_viewport(&self) -> Position {
        self.inner.get_viewport()
    }
    fn get_media_features(&self) -> MediaFeatures {
        self.inner.get_media_features()
    }
}

/// Fetch all pending images from the network and load them into the container.
/// Returns the number of images fetched.
fn fetch_images(container: &mut BrowseContainer) -> usize {
    let pending = container.inner.take_pending_images();
    let count = pending.len();
    for (src, _redraw) in &pending {
        // Use the stored baseurl context for resolution (matches litebrowser behavior)
        let baseurl = container
            .image_baseurls
            .borrow()
            .get(src.as_str())
            .cloned()
            .unwrap_or_default();
        let resolved = match container.resolve_against(src, &baseurl) {
            Some(u) => u,
            None => continue,
        };
        eprintln!("  IMG: {}", resolved);
        if let Some(data) = container.fetch_url(&resolved) {
            container.inner.load_image_data(src, &data);
        }
    }
    count
}

/// Convert premultiplied RGBA pixels to 0xRRGGBB composited against white.
fn premul_to_rgb(pixels: &[u8]) -> Vec<u32> {
    pixels
        .chunks_exact(4)
        .map(|px| {
            let (r, g, b, a) = (px[0] as u32, px[1] as u32, px[2] as u32, px[3] as u32);
            let r = (r + (255 * (255 - a) + 127) / 255).min(255);
            let g = (g + (255 * (255 - a) + 127) / 255).min(255);
            let b = (b + (255 * (255 - a) + 127) / 255).min(255);
            (r << 16) | (g << 8) | b
        })
        .collect()
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!(
            "Usage: {} <url> [width] [--height N] [--scale N] [--fullscreen]",
            args[0]
        );
        process::exit(1);
    }

    let raw_url = &args[1];
    // Collect positions of flag values so the positional width parser skips them
    let flag_value_positions: Vec<usize> = args
        .iter()
        .enumerate()
        .filter_map(|(i, a)| {
            if matches!(a.as_str(), "--height" | "--scale") {
                Some(i + 1)
            } else {
                None
            }
        })
        .collect();

    let width: u32 = args
        .iter()
        .enumerate()
        .skip(2)
        .find(|(i, a)| !flag_value_positions.contains(i) && a.parse::<u32>().is_ok())
        .and_then(|(_, s)| s.parse().ok())
        .unwrap_or(800);
    let win_height: u32 = args
        .iter()
        .position(|a| a == "--height")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(600);

    let scale: f32 = args
        .iter()
        .position(|a| a == "--scale")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(1.0);

    let fullscreen = args.iter().any(|a| a == "--fullscreen");

    let base_url = Url::parse(raw_url).unwrap_or_else(|e| {
        eprintln!("Invalid URL '{}': {}", raw_url, e);
        process::exit(1);
    });

    // Fetch the HTML with a browser User-Agent
    eprintln!("Fetching {}...", base_url);
    let agent = ureq::Agent::config_builder()
        .timeout_connect(Some(std::time::Duration::from_secs(10)))
        .timeout_recv_body(Some(std::time::Duration::from_secs(30)))
        .user_agent(USER_AGENT)
        .build()
        .new_agent();
    let html = match agent.get(base_url.as_str()).call() {
        Ok(resp) => {
            let body = resp.into_body().read_to_vec().unwrap_or_else(|e| {
                eprintln!("Failed to read response body: {}", e);
                process::exit(1);
            });
            String::from_utf8_lossy(&body).into_owned()
        }
        Err(e) => {
            eprintln!("Failed to fetch {}: {}", base_url, e);
            process::exit(1);
        }
    };

    let phys_width = ((width as f32) * scale).ceil() as u32;
    let phys_win_height = ((win_height as f32) * scale).ceil() as u32;

    // Pass 1: parse + layout to measure content height (CSS is fetched during from_html)
    let mut container = BrowseContainer::new(base_url, width, win_height, scale);
    let mut content_height = {
        eprint!("Parsing HTML + fetching CSS...");
        let t = Instant::now();
        let result =
            if let Ok(mut doc) = litehtml::Document::from_html(&html, &mut container, None, None) {
                eprintln!(" done ({:.1}s)", t.elapsed().as_secs_f64());
                eprint!("Layout (pass 1)...");
                let t = Instant::now();
                let _ = doc.render(width as f32);
                let h = (doc.height().ceil() as u32).max(win_height);
                eprintln!(" done ({:.1}s, height={})", t.elapsed().as_secs_f64(), h);
                h
            } else {
                eprintln!(" failed");
                win_height
            };
        result
    };

    // Fetch images, then re-render until layout stabilizes.
    // Images affect layout (their intrinsic size changes element dimensions),
    // and new images may be discovered after re-layout, so we loop.
    for pass in 0..4 {
        let count = fetch_images(&mut container);
        if count == 0 {
            break;
        }
        eprint!("Layout (pass {}, {} images loaded)...", pass + 2, count);
        let t = Instant::now();
        container
            .inner
            .resize_with_scale(width, content_height, scale);
        if let Ok(mut doc) = litehtml::Document::from_html(&html, &mut container, None, None) {
            let _ = doc.render(width as f32);
            content_height = (doc.height().ceil() as u32).max(win_height);
            eprintln!(
                " done ({:.1}s, height={})",
                t.elapsed().as_secs_f64(),
                content_height
            );
        }
    }

    // Final draw at the stabilized content height
    eprint!("Drawing at {}x{}...", width, content_height);
    let t = Instant::now();
    container
        .inner
        .resize_with_scale(width, content_height, scale);
    if let Ok(mut doc) = litehtml::Document::from_html(&html, &mut container, None, None) {
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
    eprintln!(" done ({:.1}s)", t.elapsed().as_secs_f64());

    let base_framebuffer = premul_to_rgb(container.inner.pixels());

    // Window
    let title = format!("browse - {}", raw_url);
    let mut window = Window::new(
        &title,
        phys_width as usize,
        phys_win_height as usize,
        WindowOptions {
            resize: false,
            borderless: fullscreen,
            ..WindowOptions::default()
        },
    )
    .unwrap_or_else(|e| {
        eprintln!("Cannot create window: {}", e);
        process::exit(1);
    });

    let max_scroll = content_height.saturating_sub(win_height);
    let mut scroll_y: u32 = 0;

    eprintln!("Ready. Scroll with mouse wheel or arrow keys. ESC to quit.");

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

        // Build visible slice
        let phys_scroll_y = ((scroll_y as f32) * scale).ceil() as u32;
        let row_start = phys_scroll_y as usize * phys_width as usize;
        let row_end = (row_start + phys_win_height as usize * phys_width as usize)
            .min(base_framebuffer.len());
        let mut visible: Vec<u32> = base_framebuffer[row_start..row_end].to_vec();

        let expected = phys_win_height as usize * phys_width as usize;
        if visible.len() < expected {
            visible.resize(expected, 0x00FFFFFF);
        }

        window
            .update_with_buffer(&visible, phys_width as usize, phys_win_height as usize)
            .unwrap();
    }
}
