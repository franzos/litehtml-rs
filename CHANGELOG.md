## [0.2.4] - 2026-03-12

### Added
- Exception guards (`try/catch`) on all C++ FFI boundary functions
- Compile-time layout assertions (`static_assert`) for reinterpret_cast'd C++ types
- Tests for text selection module
- macOS support: platform-conditional C++ stdlib linkage (`c++` on macOS, `stdc++` on Linux)
- Text selection support in `browse` example
- `FontHandle` and `DrawContext` newtypes replacing bare `usize` in the API
- Type-safe enums: `FontStyle`, `ListStyleType`, `BackgroundAttachment`, `BackgroundRepeat`, `TextAlign`, `TextDecorationLine`, `TextEmphasisPosition`
- Default implementations for optional `DocumentContainer` methods (`load_image`, `get_image_size`, `draw_image`, `draw_solid_fill`, `draw_*_gradient`)
- `Document::with_container_mut()` for safely mutating container state between document operations

### Changed
- `DocumentContainer` trait: only `create_font`, `delete_font`, `text_width`, `draw_text`, `get_viewport`, and `get_media_features` are required; all other methods now have defaults
- Pre-generated bindgen output; consumers no longer need `libclang` installed (regenerate with `--features buildtime-bindgen`)

### Fixed
- Potential UB from C++ exceptions unwinding into Rust across FFI boundary
- List marker drawing mapped to wrong CSS types (disc/circle/square)

## [0.2.3] - 2026-02-19

### Added
- `browse` example: fetch and render live web pages by URL with scrolling
- `import_css` now returns an updated base URL for correct resolution of relative references within stylesheets

### Fixed
- CSS `url()` references in external stylesheets resolved against the wrong base URL

## [0.2.2] - 2026-02-19

### Added
- CSS selector queries on elements via `select_one()` and `css_escape_ident()`
- Pending image tracking: `take_pending_images()` collects URLs discovered during layout
- Anchor click and cursor state now exposed via `take_anchor_click()` and `cursor()`

## [0.2.1] - 2026-02-18

### Added
- `set_ignore_overflow_clips()` method for full-document rendering without CSS overflow clipping

### Changed
- Clip mask caching for improved rendering performance

### Fixed
- Potential panic in `draw_list_marker()` with unknown marker types

## [0.2.0] - 2026-02-16

### Changed
- Move away from Email focus, to general-purpose HTML

## [0.1.2] - 2026-02-16

### Added
- Support text selection
- Support for HiDPI rendering
 
## [0.1.1] - 2026-02-16

### Added
- Complete bindings for `font_description` and `gradient_base`

### Changed
- Better public interface and API improvements

### Fixed
- Text wrapping
- `pos.width` inversion and `skip_until_close_tag` allocation

## [0.1.0] - 2026-02-15

### Added
- Initial release
