# litehtml-rs

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
- **`email`** -- Email preprocessing pipeline: encoding detection, HTML sanitization, `data:`/`cid:` image resolution, attribute preprocessing.

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
cargo test --workspace --features 'litehtml/pixbuf,litehtml/email'
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

## Email rendering

The `email` feature provides a full preprocessing pipeline for rendering email HTML:

```rust
use litehtml::email::{prepare_email_html, EMAIL_MASTER_CSS};

let prepared = prepare_email_html(raw_bytes, Some(&cid_resolver));
// prepared.html -- sanitized UTF-8 HTML
// prepared.images -- resolved data:/cid: images
```

This handles encoding detection (UTF-8, Windows-1252, ISO-8859-1), strips dangerous elements (`<script>`, `<iframe>`, event handlers), resolves inline images, and preprocesses legacy email attributes (`bgcolor` on `<body>`, `cellpadding`).

Remote image fetching is off by default - tracking pixels are the norm in marketing email. Pass a `url_fetcher` closure to `prepare_email_html` to opt in with your own HTTP client.
