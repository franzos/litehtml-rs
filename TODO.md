# Vello GPU Rendering Backend

## Cargo.toml
- [ ] Add `vello` feature flag: `vello = ["dep:vello", "dep:parley", "dep:image"]`
- [ ] Add `vello = { version = "0.7", optional = true }`
- [ ] Add `parley = { version = "0.7", optional = true }`
- [ ] Add dev-deps: `pollster = "0.4"`, `winit = "0.30"`, `wgpu = "27"`
- [ ] Add `[[example]] name = "render_vello"` with `required-features = ["vello"]`

## lib.rs
- [ ] Add `#[cfg(feature = "vello")] pub mod vello_backend;`

## vello_backend.rs — Struct + Constructors
- [ ] `VelloContainer` struct (scene, font_ctx, layout_ctx, fonts, clip_stack, images, viewport, etc.)
- [ ] `new(width, height)` / `new_with_scale(width, height, scale)`
- [ ] `scene(&self)` / `into_scene(self)` / `clear(&mut self)`
- [ ] `resize(&mut self, width, height)`
- [ ] `load_image_data(&mut self, url, data)` — decode + store as `peniko::Image`
- [ ] `scale_factor(&self)` getter

## vello_backend.rs — Font Management
- [ ] `FontData` internal struct (family, size, weight, style, metrics)
- [ ] `create_font` — store metadata, measure metrics via parley Layout
- [ ] `delete_font` — remove from HashMap
- [ ] `text_width` — build parley Layout, return `layout.width()`

## vello_backend.rs — Text Drawing
- [ ] `draw_text` — build parley Layout, iterate glyph runs, `scene.draw_glyphs()`

## vello_backend.rs — Fill / Gradient / Image
- [ ] `draw_solid_fill` — `scene.fill()` with solid brush on rounded rect
- [ ] `draw_linear_gradient` — `Gradient::new_linear().with_stops()` → `scene.fill()`
- [ ] `draw_radial_gradient` — `Gradient::new_radial().with_stops()` → `scene.fill()`
- [ ] `draw_conic_gradient` — `Gradient::new_sweep().with_stops()` → `scene.fill()`
- [ ] `draw_image` — `scene.draw_image()` with transform

## vello_backend.rs — Borders + Clipping
- [ ] `draw_borders` — build `kurbo::BezPath` per side, `scene.stroke()` with dash patterns
- [ ] `draw_list_marker` — disc/circle/square fills, numbered via `draw_text`
- [ ] `set_clip` — `scene.push_layer(Mix::Clip, ...)` or `push_clip_layer`
- [ ] `del_clip` — `scene.pop_layer()`

## vello_backend.rs — Remaining Trait Methods
- [ ] `set_caption` / `set_base_url` / `on_anchor_click` / `set_cursor`
- [ ] `get_viewport` — return stored viewport
- [ ] `get_media_features` — return features with scale-adjusted resolution
- [ ] `transform_text` — uppercase/lowercase/capitalize
- [ ] `load_image` / `get_image_size`

## vello_backend.rs — Headless Helper
- [ ] `render_to_rgba(html, width, height)` — wgpu headless → RGBA buffer
- [ ] `render_to_rgba_scaled(html, width, height, scale)` — same with scale factor

## Example
- [ ] `render_vello.rs` — winit window, wgpu surface, `vello::Renderer::render_to_surface()`

## Verification
- [ ] `cargo check --features vello` compiles
- [ ] Example renders sample HTML in a window
