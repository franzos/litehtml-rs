# litehtml-rs

[![crates.io](https://img.shields.io/crates/v/litehtml.svg)](https://crates.io/crates/litehtml)

Rust bindings for [litehtml](https://github.com/litehtml/litehtml) -- a lightweight HTML/CSS rendering engine.

## Crates

| Crate | Description |
|-------|-------------|
| `litehtml-sys` | Raw FFI bindings via a C wrapper (litehtml is C++) |
| `litehtml` | Safe Rust API with `DocumentContainer` trait |

## Features

The `litehtml` crate has these feature flags:

- **`vendored`** (default) -- compile litehtml from bundled source. Disable to link against a system-installed litehtml (set `LITEHTML_DIR` or ensure headers/lib are on the search path).
- **`pixbuf`** -- CPU-based pixel buffer backend using `tiny-skia` and `cosmic-text`. Gives you `PixbufContainer` and `render_to_rgba()`.
- **`html`** -- General-purpose HTML utilities: encoding detection, sanitization, `data:`/`cid:` URI resolution, legacy attribute preprocessing, and a `prepare_html` pipeline.
- **`email`** -- Email-specific defaults on top of `html`: `EMAIL_MASTER_CSS` and a `prepare_email_html` convenience wrapper.

## Usage

```rust
use litehtml::pixbuf::{PixbufContainer, render_to_rgba};

// Quick render to RGBA pixel buffer
let pixels = render_to_rgba("<p>Hello world</p>", 600, 400);

// Or use the full API for more control
let mut container = PixbufContainer::new(600, 400);
let master_css = litehtml::email::EMAIL_MASTER_CSS;

if let Some(mut doc) = Document::from_html(html, &mut container, Some(master_css), None) {
    doc.render(600.0);
    doc.draw(0, 0.0, 0.0, None);
}

let pixels = container.pixels(); // premultiplied RGBA
```

## Building

Requires a C++17 compiler and `clang` (for bindgen).

With direnv (GUIX -- see `.envrc`):

```bash
direnv allow
cargo test --workspace --features 'litehtml/pixbuf,litehtml/html'
```

Without direnv:

```bash
# Ubuntu/Debian
sudo apt-get install libclang-dev
cargo test --workspace --all-features

# GUIX (manual)
guix shell -m manifest.scm -- sh -c \
  "CC=gcc LIBCLANG_PATH=$(dirname $(find $(guix build clang-toolchain) -name 'libclang.so' | head -1)) \
  cargo test --workspace --all-features"
```

## Examples

Sample HTML files are in `examples/`. To view them in a window:

```bash
cargo run --example render --features pixbuf -p litehtml -- examples/article.html
cargo run --example render --features pixbuf -p litehtml -- examples/email.html 600
```

Scroll with mouse wheel, arrow keys, Page Up/Down, Home/End. Escape to close. Optional second argument sets the viewport width (default 800).

## HTML preprocessing

The `html` feature provides a full preprocessing pipeline for rendering HTML:

```rust
use litehtml::html::{prepare_html, sanitize_html, decode_html};

let prepared = prepare_html(raw_bytes, Some(&cid_resolver), None);
// prepared.html -- sanitized UTF-8 HTML
// prepared.images -- resolved data:/cid: images
```

This handles encoding detection (UTF-8, Windows-1252, ISO-8859-1), strips dangerous elements (`<script>`, `<iframe>`, event handlers), resolves inline images, and preprocesses legacy attributes (`bgcolor` on `<body>`, `cellpadding`).

Remote image fetching is off by default. Pass a `url_fetcher` closure as the third argument to opt in with your own HTTP client.

## Email rendering

The `email` feature adds email-specific defaults on top of `html`:

```rust
use litehtml::email::{prepare_email_html, EMAIL_MASTER_CSS};

let prepared = prepare_email_html(raw_bytes, Some(&cid_resolver), None);
```

`EMAIL_MASTER_CSS` provides an email user-agent stylesheet (body reset, responsive images, table normalization, MSO workarounds).

## Tips

A few things worth knowing when integrating `PixbufContainer` into a GUI:

- **Image loading is your job.** During layout, litehtml discovers `<img>` URLs and queues them. Call `take_pending_images()` to drain the queue, fetch the data yourself, then feed it back via `load_image_data()`. Stage multiple images before re-rendering to avoid one rebuild per image.
- **Pixel data is premultiplied.** `container.pixels()` returns premultiplied RGBA. If your framework expects straight alpha (e.g. iced's `image::Handle::from_rgba`), you'll need to unpremultiply first.
- **Anchor clicks and cursor state are pull-based.** After mouse events, call `take_anchor_click()` to check if a link was clicked and `cursor()` to read the current CSS cursor value. Discard anchor clicks during active text selections to avoid accidental navigation.
