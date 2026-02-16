# litehtml-sys

[![crates.io](https://img.shields.io/crates/v/litehtml-sys.svg)](https://crates.io/crates/litehtml-sys)

Raw FFI bindings for [litehtml](https://github.com/litehtml/litehtml) — a lightweight C++ HTML/CSS rendering engine.

This crate provides auto-generated bindings via `bindgen` through a C wrapper (litehtml is C++). For a safe Rust API, use the [`litehtml`](https://crates.io/crates/litehtml) crate instead.

## Features

- **`vendored`** (default) — compile litehtml from bundled source
- Disable to link against a system-installed litehtml (set `LITEHTML_DIR` or ensure headers/lib are on the search path)

## Requirements

C++17 compiler and `clang` (for bindgen).
