//! Pixel buffer rendering backend using `tiny-skia` for drawing and
//! `cosmic-text` for font shaping and glyph rasterization.
//!
//! Gated behind the `pixbuf` feature flag.

#![cfg(feature = "pixbuf")]

use std::cell::RefCell;
use std::collections::HashMap;

use cosmic_text::{Attrs, Family, Metrics, Shaping, Style, Weight};
use tiny_skia::{
    FillRule, GradientStop, Paint, PathBuilder, Rect, Shader, SpreadMode, Stroke, StrokeDash,
    Transform,
};

use crate::{
    BackgroundLayer, BorderRadiuses, BorderStyle, Borders, Color, ColorPoint, ConicGradient,
    DocumentContainer, FontDescription, FontMetrics, LinearGradient, ListMarker, MediaFeatures,
    MediaType, Position, RadialGradient, Size, TextTransform,
};

/// Internal font data associated with a font handle.
struct FontData {
    family: String,
    size: f32,
    weight: Weight,
    style: Style,
    metrics: FontMetrics,
}

/// A pixel buffer rendering backend that implements [`DocumentContainer`].
///
/// Uses `tiny-skia` for 2D drawing primitives and `cosmic-text` for text
/// shaping, layout, and glyph rasterization. All rendering happens on the
/// CPU into an in-memory RGBA pixel buffer.
pub struct PixbufContainer {
    pixmap: tiny_skia::Pixmap,
    // RefCell because `text_width` takes `&self` but cosmic-text needs `&mut`
    font_system: RefCell<cosmic_text::FontSystem>,
    swash_cache: RefCell<cosmic_text::SwashCache>,
    fonts: HashMap<usize, FontData>,
    next_font_id: usize,
    clip_stack: Vec<(Position, BorderRadiuses)>,
    images: HashMap<String, tiny_skia::Pixmap>,
    viewport: Position,
    base_url: String,
    caption: String,
}

impl PixbufContainer {
    /// Create a new pixel buffer container with the given dimensions.
    ///
    /// Initializes a transparent pixmap and loads system fonts via cosmic-text.
    pub fn new(width: u32, height: u32) -> Self {
        let pixmap =
            tiny_skia::Pixmap::new(width.max(1), height.max(1)).expect("failed to create pixmap");
        Self {
            pixmap,
            font_system: RefCell::new(cosmic_text::FontSystem::new()),
            swash_cache: RefCell::new(cosmic_text::SwashCache::new()),
            fonts: HashMap::new(),
            next_font_id: 1,
            clip_stack: Vec::new(),
            images: HashMap::new(),
            viewport: Position {
                x: 0.0,
                y: 0.0,
                width: width as f32,
                height: height as f32,
            },
            base_url: String::new(),
            caption: String::new(),
        }
    }

    /// Get the rendered pixel data as premultiplied RGBA bytes.
    pub fn pixels(&self) -> &[u8] {
        self.pixmap.data()
    }

    /// Get the pixmap width.
    pub fn width(&self) -> u32 {
        self.pixmap.width()
    }

    /// Get the pixmap height.
    pub fn height(&self) -> u32 {
        self.pixmap.height()
    }

    /// Load an image from raw bytes, decoded with the `image` crate.
    ///
    /// The decoded pixels are stored internally and referenced by `url` during
    /// subsequent draw calls.
    pub fn load_image_data(&mut self, url: &str, data: &[u8]) {
        let Ok(img) = image::load_from_memory(data) else {
            return;
        };
        let rgba = img.to_rgba8();
        let (w, h) = (rgba.width(), rgba.height());

        // tiny-skia expects premultiplied alpha
        let mut premul = rgba.into_raw();
        for chunk in premul.chunks_exact_mut(4) {
            let a = chunk[3] as u32;
            chunk[0] = ((chunk[0] as u32 * a + 127) / 255) as u8;
            chunk[1] = ((chunk[1] as u32 * a + 127) / 255) as u8;
            chunk[2] = ((chunk[2] as u32 * a + 127) / 255) as u8;
        }

        if let Some(pm) = tiny_skia::Pixmap::from_vec(
            premul,
            tiny_skia::IntSize::from_wh(w, h).expect("invalid image size"),
        ) {
            self.images.insert(url.to_string(), pm);
        }
    }

    /// Resize the pixmap, clearing all existing content.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.pixmap =
            tiny_skia::Pixmap::new(width.max(1), height.max(1)).expect("failed to create pixmap");
        self.viewport.width = width as f32;
        self.viewport.height = height as f32;
    }

    /// Build a clip mask from the current clip stack.
    fn build_clip_mask(&self) -> Option<tiny_skia::Mask> {
        if self.clip_stack.is_empty() {
            return None;
        }

        let w = self.pixmap.width();
        let h = self.pixmap.height();
        let mut mask = tiny_skia::Mask::new(w, h)?;

        // Start fully opaque
        mask.fill_path(
            &PathBuilder::from_rect(Rect::from_xywh(0.0, 0.0, w as f32, h as f32)?),
            FillRule::Winding,
            true,
            Transform::identity(),
        );

        // Intersect each clip rect
        for (pos, radii) in &self.clip_stack {
            let mut clip_mask = tiny_skia::Mask::new(w, h)?;
            let path = build_rounded_rect_path(pos.x, pos.y, pos.width, pos.height, radii);
            if let Some(path) = path {
                clip_mask.fill_path(&path, FillRule::Winding, true, Transform::identity());
            }
            // Intersect: combine masks by taking minimum
            intersect_masks(&mut mask, &clip_mask);
        }

        Some(mask)
    }

    /// Create a `Paint` with a solid color.
    fn solid_paint(color: Color) -> Paint<'static> {
        Paint {
            shader: Shader::SolidColor(tiny_skia::Color::from_rgba8(
                color.r, color.g, color.b, color.a,
            )),
            anti_alias: true,
            ..Paint::default()
        }
    }

    /// Create cosmic-text Attrs from internal font data.
    fn attrs_from_font<'a>(font: &'a FontData) -> Attrs<'a> {
        let family = match font.family.as_str() {
            "serif" => Family::Serif,
            "sans-serif" | "sans serif" => Family::SansSerif,
            "monospace" => Family::Monospace,
            "cursive" => Family::Cursive,
            "fantasy" => Family::Fantasy,
            name => Family::Name(name),
        };
        Attrs::new()
            .family(family)
            .weight(font.weight)
            .style(font.style)
    }

    /// Measure a string of text using cosmic-text, returning total width.
    fn measure_text(&self, text: &str, font: &FontData) -> f32 {
        let mut fs = self.font_system.borrow_mut();
        let line_height = font.metrics.height;
        let metrics = Metrics::new(font.size, line_height);
        let mut buffer = cosmic_text::Buffer::new(&mut fs, metrics);
        buffer.set_size(&mut fs, Some(f32::MAX), Some(line_height));
        let attrs = Self::attrs_from_font(font);
        buffer.set_text(&mut fs, text, &attrs, Shaping::Advanced);
        buffer.shape_until_scroll(&mut fs, false);

        buffer.layout_runs().map(|run| run.line_w).sum::<f32>()
    }
}

/// Intersect two masks by taking the minimum alpha of each pixel.
fn intersect_masks(dst: &mut tiny_skia::Mask, src: &tiny_skia::Mask) {
    let dst_data = dst.data_mut();
    let src_data = src.data();
    let len = dst_data.len().min(src_data.len());
    for i in 0..len {
        dst_data[i] = dst_data[i].min(src_data[i]);
    }
}

/// Build a rounded rectangle path. Falls back to a plain rect if all radii
/// are zero.
fn build_rounded_rect_path(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    radii: &BorderRadiuses,
) -> Option<tiny_skia::Path> {
    if w <= 0.0 || h <= 0.0 {
        return None;
    }

    let has_radii = radii.top_left_x > 0.0
        || radii.top_left_y > 0.0
        || radii.top_right_x > 0.0
        || radii.top_right_y > 0.0
        || radii.bottom_right_x > 0.0
        || radii.bottom_right_y > 0.0
        || radii.bottom_left_x > 0.0
        || radii.bottom_left_y > 0.0;

    if !has_radii {
        return Rect::from_xywh(x, y, w, h).map(|r| PathBuilder::from_rect(r));
    }

    // Clamp radii to half the dimension so corners don't overlap
    let max_rx = w / 2.0;
    let max_ry = h / 2.0;

    let tl_x = radii.top_left_x.min(max_rx);
    let tl_y = radii.top_left_y.min(max_ry);
    let tr_x = radii.top_right_x.min(max_rx);
    let tr_y = radii.top_right_y.min(max_ry);
    let br_x = radii.bottom_right_x.min(max_rx);
    let br_y = radii.bottom_right_y.min(max_ry);
    let bl_x = radii.bottom_left_x.min(max_rx);
    let bl_y = radii.bottom_left_y.min(max_ry);

    // Magic number for approximating a quarter circle with a cubic bezier
    const K: f32 = 0.552_284_75;

    let mut pb = PathBuilder::new();

    // Start at top-left, after the TL corner
    pb.move_to(x + tl_x, y);

    // Top edge -> top-right corner
    pb.line_to(x + w - tr_x, y);
    if tr_x > 0.0 || tr_y > 0.0 {
        pb.cubic_to(
            x + w - tr_x * (1.0 - K),
            y,
            x + w,
            y + tr_y * (1.0 - K),
            x + w,
            y + tr_y,
        );
    }

    // Right edge -> bottom-right corner
    pb.line_to(x + w, y + h - br_y);
    if br_x > 0.0 || br_y > 0.0 {
        pb.cubic_to(
            x + w,
            y + h - br_y * (1.0 - K),
            x + w - br_x * (1.0 - K),
            y + h,
            x + w - br_x,
            y + h,
        );
    }

    // Bottom edge -> bottom-left corner
    pb.line_to(x + bl_x, y + h);
    if bl_x > 0.0 || bl_y > 0.0 {
        pb.cubic_to(
            x + bl_x * (1.0 - K),
            y + h,
            x,
            y + h - bl_y * (1.0 - K),
            x,
            y + h - bl_y,
        );
    }

    // Left edge -> top-left corner
    pb.line_to(x, y + tl_y);
    if tl_x > 0.0 || tl_y > 0.0 {
        pb.cubic_to(
            x,
            y + tl_y * (1.0 - K),
            x + tl_x * (1.0 - K),
            y,
            x + tl_x,
            y,
        );
    }

    pb.close();
    pb.finish()
}

/// Convert a litehtml color + offset pair to tiny-skia gradient stops.
fn color_points_to_stops(points: &[ColorPoint]) -> Vec<GradientStop> {
    points
        .iter()
        .map(|cp| {
            let pos = cp.offset.clamp(0.0, 1.0);
            let color =
                tiny_skia::Color::from_rgba8(cp.color.r, cp.color.g, cp.color.b, cp.color.a);
            GradientStop::new(pos, color)
        })
        .collect()
}

impl DocumentContainer for PixbufContainer {
    fn create_font(&mut self, descr: &FontDescription) -> (usize, FontMetrics) {
        let family_str = descr.family().to_string();
        let size = descr.size();

        let weight = Weight(descr.weight() as u16);
        let style = match descr.style() {
            1 => Style::Italic,
            2 => Style::Oblique,
            _ => Style::Normal,
        };

        let id = self.next_font_id;
        self.next_font_id += 1;

        // Use cosmic-text to measure reference characters for metrics
        let line_height = (size * 1.2).ceil();
        let ct_metrics = Metrics::new(size, line_height);

        let font_family = match family_str.as_str() {
            "serif" => Family::Serif,
            "sans-serif" | "sans serif" => Family::SansSerif,
            "monospace" => Family::Monospace,
            "cursive" => Family::Cursive,
            "fantasy" => Family::Fantasy,
            name => Family::Name(name),
        };

        let attrs = Attrs::new().family(font_family).weight(weight).style(style);

        let mut fs = self.font_system.borrow_mut();

        // Measure "x" for x_height
        let x_height = {
            let mut buf = cosmic_text::Buffer::new(&mut fs, ct_metrics);
            buf.set_size(&mut fs, Some(f32::MAX), Some(line_height));
            buf.set_text(&mut fs, "x", &attrs, Shaping::Advanced);
            buf.shape_until_scroll(&mut fs, false);

            let mut h = size * 0.5; // fallback
            for run in buf.layout_runs() {
                for glyph in run.glyphs.iter() {
                    let physical = glyph.physical((0.0, 0.0), 1.0);
                    let mut sc = self.swash_cache.borrow_mut();
                    if let Some(img) = sc.get_image_uncached(&mut fs, physical.cache_key) {
                        h = img.placement.height as f32;
                    }
                    break;
                }
                break;
            }
            h
        };

        // Measure "0" for ch_width
        let ch_width = {
            let mut buf = cosmic_text::Buffer::new(&mut fs, ct_metrics);
            buf.set_size(&mut fs, Some(f32::MAX), Some(line_height));
            buf.set_text(&mut fs, "0", &attrs, Shaping::Advanced);
            buf.shape_until_scroll(&mut fs, false);

            buf.layout_runs()
                .flat_map(|run| run.glyphs.iter())
                .map(|g| g.w)
                .next()
                .unwrap_or(size * 0.6)
        };

        let ascent = size * 0.8;
        let descent = size * 0.2;

        let metrics = FontMetrics {
            font_size: size,
            height: line_height,
            ascent,
            descent,
            x_height,
            ch_width,
            draw_spaces: true,
            sub_shift: size * 0.3,
            super_shift: size * 0.4,
        };

        self.fonts.insert(
            id,
            FontData {
                family: family_str,
                size,
                weight,
                style,
                metrics,
            },
        );

        (id, metrics)
    }

    fn delete_font(&mut self, font: usize) {
        self.fonts.remove(&font);
    }

    fn text_width(&self, text: &str, font: usize) -> f32 {
        let Some(font_data) = self.fonts.get(&font) else {
            return text.len() as f32 * 8.0;
        };
        self.measure_text(text, font_data)
    }

    fn draw_text(&mut self, _hdc: usize, text: &str, font: usize, color: Color, pos: Position) {
        let Some(font_data) = self.fonts.get(&font) else {
            return;
        };

        let line_height = font_data.metrics.height;
        let ct_metrics = Metrics::new(font_data.size, line_height);
        let attrs = Self::attrs_from_font(font_data);
        let mask = self.build_clip_mask();

        let mut fs = self.font_system.borrow_mut();
        let mut buffer = cosmic_text::Buffer::new(&mut fs, ct_metrics);
        buffer.set_size(
            &mut fs,
            Some(pos.width.min(f32::MAX / 2.0)),
            Some(line_height),
        );
        buffer.set_text(&mut fs, text, &attrs, Shaping::Advanced);
        buffer.shape_until_scroll(&mut fs, false);

        let mut swash = self.swash_cache.borrow_mut();

        let draw_x = pos.x as i32;
        let draw_y = pos.y as i32;
        let pix_w = self.pixmap.width() as i32;
        let pix_h = self.pixmap.height() as i32;

        for run in buffer.layout_runs() {
            let baseline_y = run.line_y as i32;
            for glyph in run.glyphs.iter() {
                let physical = glyph.physical((0.0, 0.0), 1.0);

                if let Some(image) = swash.get_image_uncached(&mut fs, physical.cache_key) {
                    let gx = draw_x + physical.x + image.placement.left;
                    let gy = draw_y + baseline_y + physical.y - image.placement.top;

                    match image.content {
                        cosmic_text::SwashContent::Mask => {
                            // Alpha mask: blend using the text color
                            let mut i = 0;
                            for off_y in 0..image.placement.height as i32 {
                                for off_x in 0..image.placement.width as i32 {
                                    let px = gx + off_x;
                                    let py = gy + off_y;
                                    if px >= 0 && px < pix_w && py >= 0 && py < pix_h {
                                        let alpha = image.data[i];
                                        if alpha > 0 {
                                            // Blend with text color at this alpha
                                            let a = (alpha as u32 * color.a as u32 + 127) / 255;
                                            blend_pixel(
                                                self.pixmap.data_mut(),
                                                pix_w as u32,
                                                px as u32,
                                                py as u32,
                                                color.r,
                                                color.g,
                                                color.b,
                                                a as u8,
                                                mask.as_ref(),
                                            );
                                        }
                                    }
                                    i += 1;
                                }
                            }
                        }
                        cosmic_text::SwashContent::Color => {
                            // RGBA color glyphs (emoji, etc.)
                            let mut i = 0;
                            for off_y in 0..image.placement.height as i32 {
                                for off_x in 0..image.placement.width as i32 {
                                    let px = gx + off_x;
                                    let py = gy + off_y;
                                    if px >= 0 && px < pix_w && py >= 0 && py < pix_h {
                                        let r = image.data[i];
                                        let g = image.data[i + 1];
                                        let b = image.data[i + 2];
                                        let a = image.data[i + 3];
                                        if a > 0 {
                                            blend_pixel(
                                                self.pixmap.data_mut(),
                                                pix_w as u32,
                                                px as u32,
                                                py as u32,
                                                r,
                                                g,
                                                b,
                                                a,
                                                mask.as_ref(),
                                            );
                                        }
                                    }
                                    i += 4;
                                }
                            }
                        }
                        cosmic_text::SwashContent::SubpixelMask => {
                            // Not supported, treat as regular mask using luminance
                            let mut i = 0;
                            for off_y in 0..image.placement.height as i32 {
                                for off_x in 0..image.placement.width as i32 {
                                    let px = gx + off_x;
                                    let py = gy + off_y;
                                    if px >= 0 && px < pix_w && py >= 0 && py < pix_h {
                                        // Use green channel as alpha approximation
                                        let alpha = if i + 2 < image.data.len() {
                                            image.data[i + 1]
                                        } else {
                                            0
                                        };
                                        if alpha > 0 {
                                            let a = (alpha as u32 * color.a as u32 + 127) / 255;
                                            blend_pixel(
                                                self.pixmap.data_mut(),
                                                pix_w as u32,
                                                px as u32,
                                                py as u32,
                                                color.r,
                                                color.g,
                                                color.b,
                                                a as u8,
                                                mask.as_ref(),
                                            );
                                        }
                                    }
                                    i += 3;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn draw_list_marker(&mut self, _hdc: usize, marker: &ListMarker) {
        let pos = marker.pos();
        let color = marker.color();
        let marker_type = marker.marker_type();
        let paint = Self::solid_paint(color);
        let mask = self.build_clip_mask();

        // Marker types: disc=0, circle=1, square=2, others are numbered
        match marker_type {
            0 => {
                // Disc: filled circle
                let cx = pos.x + pos.width / 2.0;
                let cy = pos.y + pos.height / 2.0;
                let r = pos.width.min(pos.height) / 2.0;
                if let Some(path) = build_circle_path(cx, cy, r) {
                    self.pixmap.fill_path(
                        &path,
                        &paint,
                        FillRule::Winding,
                        Transform::identity(),
                        mask.as_ref(),
                    );
                }
            }
            1 => {
                // Circle: stroked circle
                let cx = pos.x + pos.width / 2.0;
                let cy = pos.y + pos.height / 2.0;
                let r = pos.width.min(pos.height) / 2.0;
                if let Some(path) = build_circle_path(cx, cy, r) {
                    let stroke = Stroke {
                        width: 1.0,
                        ..Stroke::default()
                    };
                    self.pixmap.stroke_path(
                        &path,
                        &paint,
                        &stroke,
                        Transform::identity(),
                        mask.as_ref(),
                    );
                }
            }
            2 => {
                // Square: filled rectangle
                if let Some(rect) = Rect::from_xywh(pos.x, pos.y, pos.width, pos.height) {
                    self.pixmap
                        .fill_rect(rect, &paint, Transform::identity(), mask.as_ref());
                }
            }
            _ => {
                // Numbered marker: draw the index as text
                let idx = marker.index();
                let text = format!("{}.", idx);
                let font_id = marker.font();
                self.draw_text(0, &text, font_id, color, pos);
            }
        }
    }

    fn load_image(&mut self, _src: &str, _baseurl: &str, _redraw_on_ready: bool) {
        // Image loading is handled externally via `load_image_data`.
    }

    fn get_image_size(&self, src: &str, _baseurl: &str) -> Size {
        if let Some(pm) = self.images.get(src) {
            Size {
                width: pm.width() as f32,
                height: pm.height() as f32,
            }
        } else {
            Size::default()
        }
    }

    fn draw_image(&mut self, _hdc: usize, layer: &BackgroundLayer, url: &str, _base_url: &str) {
        let Some(img) = self.images.get(url) else {
            return;
        };
        let clip = layer.clip_box();
        let border = layer.border_box();
        let mask = self.build_clip_mask();

        // Determine source and destination
        let dst_x = border.x as i32;
        let dst_y = border.y as i32;

        let img_paint = tiny_skia::PixmapPaint {
            opacity: 1.0,
            blend_mode: tiny_skia::BlendMode::SourceOver,
            quality: tiny_skia::FilterQuality::Bilinear,
        };

        // Use clip_box to limit drawing area via a clip mask
        let combined_mask = if clip.width > 0.0 && clip.height > 0.0 {
            let w = self.pixmap.width();
            let h = self.pixmap.height();
            let mut m = tiny_skia::Mask::new(w, h);
            if let Some(ref mut m) = m {
                if let Some(rect) = Rect::from_xywh(clip.x, clip.y, clip.width, clip.height) {
                    m.fill_path(
                        &PathBuilder::from_rect(rect),
                        FillRule::Winding,
                        true,
                        Transform::identity(),
                    );
                }
                // Intersect with existing clip mask
                if let Some(ref existing) = mask {
                    intersect_masks(m, existing);
                }
            }
            m
        } else {
            mask
        };

        self.pixmap.draw_pixmap(
            dst_x,
            dst_y,
            img.as_ref(),
            &img_paint,
            Transform::identity(),
            combined_mask.as_ref(),
        );
    }

    fn draw_solid_fill(&mut self, _hdc: usize, layer: &BackgroundLayer, color: Color) {
        if color.a == 0 {
            return;
        }
        let border = layer.border_box();
        let radii = layer.border_radius();
        let paint = Self::solid_paint(color);
        let mask = self.build_clip_mask();

        if let Some(path) =
            build_rounded_rect_path(border.x, border.y, border.width, border.height, &radii)
        {
            self.pixmap.fill_path(
                &path,
                &paint,
                FillRule::Winding,
                Transform::identity(),
                mask.as_ref(),
            );
        }
    }

    fn draw_linear_gradient(
        &mut self,
        _hdc: usize,
        layer: &BackgroundLayer,
        gradient: &LinearGradient,
    ) {
        let border = layer.border_box();
        let radii = layer.border_radius();
        let start = gradient.start();
        let end = gradient.end();
        let points = gradient.color_points();
        let stops = color_points_to_stops(&points);
        let mask = self.build_clip_mask();

        if stops.len() < 2 {
            // Fall back to solid color with the first stop
            if let Some(cp) = points.first() {
                self.draw_solid_fill(0, layer, cp.color);
            }
            return;
        }

        let shader = tiny_skia::LinearGradient::new(
            tiny_skia::Point::from_xy(border.x + start.x, border.y + start.y),
            tiny_skia::Point::from_xy(border.x + end.x, border.y + end.y),
            stops,
            SpreadMode::Pad,
            Transform::identity(),
        );

        if let Some(shader) = shader {
            let paint = Paint {
                shader,
                anti_alias: true,
                ..Paint::default()
            };

            if let Some(path) =
                build_rounded_rect_path(border.x, border.y, border.width, border.height, &radii)
            {
                self.pixmap.fill_path(
                    &path,
                    &paint,
                    FillRule::Winding,
                    Transform::identity(),
                    mask.as_ref(),
                );
            }
        }
    }

    fn draw_radial_gradient(
        &mut self,
        _hdc: usize,
        layer: &BackgroundLayer,
        gradient: &RadialGradient,
    ) {
        let border = layer.border_box();
        let radii = layer.border_radius();
        let center = gradient.position();
        let radius = gradient.radius();
        let points = gradient.color_points();
        let stops = color_points_to_stops(&points);
        let mask = self.build_clip_mask();

        if stops.len() < 2 {
            if let Some(cp) = points.first() {
                self.draw_solid_fill(0, layer, cp.color);
            }
            return;
        }

        let cx = border.x + center.x;
        let cy = border.y + center.y;
        let r = radius.x.max(radius.y).max(0.001);

        let shader = tiny_skia::RadialGradient::new(
            tiny_skia::Point::from_xy(cx, cy),
            tiny_skia::Point::from_xy(cx, cy),
            r,
            stops,
            SpreadMode::Pad,
            Transform::identity(),
        );

        if let Some(shader) = shader {
            let paint = Paint {
                shader,
                anti_alias: true,
                ..Paint::default()
            };

            if let Some(path) =
                build_rounded_rect_path(border.x, border.y, border.width, border.height, &radii)
            {
                self.pixmap.fill_path(
                    &path,
                    &paint,
                    FillRule::Winding,
                    Transform::identity(),
                    mask.as_ref(),
                );
            }
        }
    }

    fn draw_conic_gradient(
        &mut self,
        _hdc: usize,
        layer: &BackgroundLayer,
        gradient: &ConicGradient,
    ) {
        // Conic gradients are not natively supported by tiny-skia.
        // Fill with the first color stop as a fallback.
        let points = gradient.color_points();
        if let Some(cp) = points.first() {
            self.draw_solid_fill(0, layer, cp.color);
        }
    }

    fn draw_borders(&mut self, _hdc: usize, borders: &Borders, draw_pos: Position, _root: bool) {
        let mask = self.build_clip_mask();
        let x = draw_pos.x;
        let y = draw_pos.y;
        let w = draw_pos.width;
        let h = draw_pos.height;

        // Draw each border side
        draw_border_side(
            &mut self.pixmap,
            mask.as_ref(),
            &borders.top,
            x,
            y,
            w,
            borders.top.width,
            true,
        );

        draw_border_side(
            &mut self.pixmap,
            mask.as_ref(),
            &borders.bottom,
            x,
            y + h - borders.bottom.width,
            w,
            borders.bottom.width,
            true,
        );

        draw_border_side(
            &mut self.pixmap,
            mask.as_ref(),
            &borders.left,
            x,
            y,
            borders.left.width,
            h,
            false,
        );

        draw_border_side(
            &mut self.pixmap,
            mask.as_ref(),
            &borders.right,
            x + w - borders.right.width,
            y,
            borders.right.width,
            h,
            false,
        );
    }

    fn set_caption(&mut self, caption: &str) {
        self.caption = caption.to_string();
    }

    fn set_base_url(&mut self, base_url: &str) {
        self.base_url = base_url.to_string();
    }

    fn on_anchor_click(&mut self, _url: &str) {}

    fn set_cursor(&mut self, _cursor: &str) {}

    fn set_clip(&mut self, pos: Position, radius: BorderRadiuses) {
        self.clip_stack.push((pos, radius));
    }

    fn del_clip(&mut self) {
        self.clip_stack.pop();
    }

    fn get_viewport(&self) -> Position {
        self.viewport
    }

    fn get_media_features(&self) -> MediaFeatures {
        MediaFeatures {
            media_type: MediaType::Screen,
            width: self.viewport.width,
            height: self.viewport.height,
            device_width: self.viewport.width,
            device_height: self.viewport.height,
            color: 8,
            color_index: 0,
            monochrome: 0,
            resolution: 96.0,
        }
    }

    fn transform_text(&self, text: &str, tt: TextTransform) -> String {
        match tt {
            TextTransform::Uppercase => text.to_uppercase(),
            TextTransform::Lowercase => text.to_lowercase(),
            TextTransform::Capitalize => {
                let mut result = String::with_capacity(text.len());
                let mut capitalize_next = true;
                for ch in text.chars() {
                    if capitalize_next && ch.is_alphabetic() {
                        for upper in ch.to_uppercase() {
                            result.push(upper);
                        }
                        capitalize_next = false;
                    } else {
                        result.push(ch);
                        if ch.is_whitespace() {
                            capitalize_next = true;
                        }
                    }
                }
                result
            }
            TextTransform::None => text.to_string(),
        }
    }
}

/// Blend a single pixel onto the pixmap data using source-over compositing.
///
/// The pixmap stores premultiplied RGBA, so we convert accordingly.
fn blend_pixel(
    data: &mut [u8],
    width: u32,
    x: u32,
    y: u32,
    r: u8,
    g: u8,
    b: u8,
    a: u8,
    mask: Option<&tiny_skia::Mask>,
) {
    if a == 0 {
        return;
    }

    // Apply clip mask
    let effective_a = if let Some(mask) = mask {
        let mask_idx = (y * width + x) as usize;
        let mask_data = mask.data();
        if mask_idx >= mask_data.len() {
            return;
        }
        let mask_val = mask_data[mask_idx];
        if mask_val == 0 {
            return;
        }
        ((a as u32 * mask_val as u32 + 127) / 255) as u8
    } else {
        a
    };

    if effective_a == 0 {
        return;
    }

    let idx = ((y * width + x) * 4) as usize;
    if idx + 3 >= data.len() {
        return;
    }

    // Source in premultiplied alpha
    let sa = effective_a as u32;
    let sr = (r as u32 * sa + 127) / 255;
    let sg = (g as u32 * sa + 127) / 255;
    let sb = (b as u32 * sa + 127) / 255;

    // Destination (already premultiplied)
    let dr = data[idx] as u32;
    let dg = data[idx + 1] as u32;
    let db = data[idx + 2] as u32;
    let da = data[idx + 3] as u32;

    // Source-over: out = src + dst * (1 - src_alpha)
    let inv_sa = 255 - sa;
    data[idx] = (sr + (dr * inv_sa + 127) / 255).min(255) as u8;
    data[idx + 1] = (sg + (dg * inv_sa + 127) / 255).min(255) as u8;
    data[idx + 2] = (sb + (db * inv_sa + 127) / 255).min(255) as u8;
    data[idx + 3] = (sa + (da * inv_sa + 127) / 255).min(255) as u8;
}

/// Draw a single border side (top, bottom, left, or right).
fn draw_border_side(
    pixmap: &mut tiny_skia::Pixmap,
    mask: Option<&tiny_skia::Mask>,
    border: &crate::Border,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    horizontal: bool,
) {
    if border.width <= 0.0 || matches!(border.style, BorderStyle::None | BorderStyle::Hidden) {
        return;
    }

    let paint = Paint {
        shader: Shader::SolidColor(tiny_skia::Color::from_rgba8(
            border.color.r,
            border.color.g,
            border.color.b,
            border.color.a,
        )),
        anti_alias: true,
        ..Paint::default()
    };

    match border.style {
        BorderStyle::Solid
        | BorderStyle::Double
        | BorderStyle::Groove
        | BorderStyle::Ridge
        | BorderStyle::Inset
        | BorderStyle::Outset => {
            if let Some(rect) = Rect::from_xywh(x, y, w.max(0.001), h.max(0.001)) {
                pixmap.fill_rect(rect, &paint, Transform::identity(), mask);
            }
        }
        BorderStyle::Dashed => {
            // Dashed border: use a stroked path with dash pattern
            let mut pb = PathBuilder::new();
            if horizontal {
                let mid_y = y + h / 2.0;
                pb.move_to(x, mid_y);
                pb.line_to(x + w, mid_y);
            } else {
                let mid_x = x + w / 2.0;
                pb.move_to(mid_x, y);
                pb.line_to(mid_x, y + h);
            }
            if let Some(path) = pb.finish() {
                let dash_len = border.width * 3.0;
                let stroke = Stroke {
                    width: border.width,
                    dash: StrokeDash::new(vec![dash_len, dash_len], 0.0),
                    ..Stroke::default()
                };
                pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), mask);
            }
        }
        BorderStyle::Dotted => {
            // Dotted border: use a stroked path with dot pattern
            let mut pb = PathBuilder::new();
            if horizontal {
                let mid_y = y + h / 2.0;
                pb.move_to(x, mid_y);
                pb.line_to(x + w, mid_y);
            } else {
                let mid_x = x + w / 2.0;
                pb.move_to(mid_x, y);
                pb.line_to(mid_x, y + h);
            }
            if let Some(path) = pb.finish() {
                let dot = border.width;
                let stroke = Stroke {
                    width: border.width,
                    line_cap: tiny_skia::LineCap::Round,
                    dash: StrokeDash::new(vec![0.001, dot * 2.0], 0.0),
                    ..Stroke::default()
                };
                pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), mask);
            }
        }
        BorderStyle::None | BorderStyle::Hidden => {}
    }
}

/// Build a circle path approximated with cubic beziers.
fn build_circle_path(cx: f32, cy: f32, r: f32) -> Option<tiny_skia::Path> {
    if r <= 0.0 {
        return None;
    }

    const K: f32 = 0.552_284_75;
    let mut pb = PathBuilder::new();

    pb.move_to(cx + r, cy);
    pb.cubic_to(cx + r, cy + r * K, cx + r * K, cy + r, cx, cy + r);
    pb.cubic_to(cx - r * K, cy + r, cx - r, cy + r * K, cx - r, cy);
    pb.cubic_to(cx - r, cy - r * K, cx - r * K, cy - r, cx, cy - r);
    pb.cubic_to(cx + r * K, cy - r, cx + r, cy - r * K, cx + r, cy);
    pb.close();
    pb.finish()
}

/// Render HTML to an RGBA pixel buffer.
///
/// This is a convenience function that creates a container, parses the HTML,
/// lays it out, and draws it, returning the raw pixel data.
pub fn render_to_rgba(html: &str, width: u32, height: u32) -> Vec<u8> {
    let mut container = PixbufContainer::new(width, height);
    if let Ok(mut doc) = crate::Document::from_html(html, &mut container, None, None) {
        let _ = doc.render(width as f32);
        doc.draw(
            0,
            0.0,
            0.0,
            Some(Position {
                x: 0.0,
                y: 0.0,
                width: width as f32,
                height: height as f32,
            }),
        );
    }
    container.pixels().to_vec()
}
