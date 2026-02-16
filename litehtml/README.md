# litehtml

[![crates.io](https://img.shields.io/crates/v/litehtml.svg)](https://crates.io/crates/litehtml)

Safe Rust bindings for [litehtml](https://github.com/litehtml/litehtml) — a lightweight HTML/CSS rendering engine.

## Features

- **`vendored`** (default) — compile litehtml from bundled source
- **`pixbuf`** — CPU-based pixel buffer rendering via `tiny-skia` and `cosmic-text`
- **`html`** — encoding detection, sanitization, URI resolution, legacy attribute preprocessing
- **`email`** — email-specific defaults on top of `html`

## Quick start

```rust
use litehtml::html::prepare_html;

let prepared = prepare_html(raw_bytes, None, None);
// prepared.html — sanitized UTF-8 HTML
// prepared.images — resolved data:/cid: images
```

See the [repository](https://github.com/franzos/litehtml-rs) for full documentation and examples.
