//! Safe Rust bindings for the litehtml HTML/CSS rendering engine.
//!
//! This crate wraps the raw C FFI provided by `litehtml-sys` with safe,
//! idiomatic Rust types. Users implement the [`DocumentContainer`] trait to
//! provide font metrics, drawing, and resource loading, then create a
//! [`Document`] to parse and render HTML.

use std::ffi::{CStr, CString};
use std::marker::PhantomData;
use std::os::raw::{c_char, c_int, c_void};
use std::panic::{catch_unwind, AssertUnwindSafe};

use log::warn;

pub use litehtml_sys as sys;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Error returned by [`Document::from_html`] when document creation fails.
#[derive(Debug)]
pub enum CreateError {
    /// The input HTML or CSS string contained an interior null byte.
    InvalidString(std::ffi::NulError),
    /// The litehtml C++ engine returned a null document pointer.
    CreateFailed,
}

impl std::fmt::Display for CreateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidString(e) => write!(f, "string contains interior null byte: {e}"),
            Self::CreateFailed => write!(f, "litehtml failed to create document"),
        }
    }
}

impl std::error::Error for CreateError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidString(e) => Some(e),
            Self::CreateFailed => None,
        }
    }
}

impl From<std::ffi::NulError> for CreateError {
    fn from(e: std::ffi::NulError) -> Self {
        Self::InvalidString(e)
    }
}

// ---------------------------------------------------------------------------
// Safe Rust value types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Position {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

/// RGBA color value.
///
/// Note: litehtml's C++ `web_color` type carries an `is_current_color` flag
/// (for the CSS `currentColor` keyword). This flag is **not** preserved here
/// because litehtml resolves `currentColor` to a concrete RGBA value during
/// CSS property computation, before passing colors to container callbacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Default for Color {
    fn default() -> Self {
        Self {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct FontMetrics {
    pub font_size: f32,
    pub height: f32,
    pub ascent: f32,
    pub descent: f32,
    pub x_height: f32,
    pub ch_width: f32,
    pub draw_spaces: bool,
    pub sub_shift: f32,
    pub super_shift: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct BorderRadiuses {
    pub top_left_x: f32,
    pub top_left_y: f32,
    pub top_right_x: f32,
    pub top_right_y: f32,
    pub bottom_right_x: f32,
    pub bottom_right_y: f32,
    pub bottom_left_x: f32,
    pub bottom_left_y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Border {
    pub width: f32,
    pub style: BorderStyle,
    pub color: Color,
}

impl Default for Border {
    fn default() -> Self {
        Self {
            width: 0.0,
            style: BorderStyle::None,
            color: Color::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Borders {
    pub left: Border,
    pub top: Border,
    pub right: Border,
    pub bottom: Border,
    pub radius: BorderRadiuses,
}

impl Default for Borders {
    fn default() -> Self {
        Self {
            left: Border::default(),
            top: Border::default(),
            right: Border::default(),
            bottom: Border::default(),
            radius: BorderRadiuses::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MediaFeatures {
    pub media_type: MediaType,
    pub width: f32,
    pub height: f32,
    pub device_width: f32,
    pub device_height: f32,
    pub color: i32,
    pub color_index: i32,
    pub monochrome: i32,
    pub resolution: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorPoint {
    pub offset: f32,
    pub color: Color,
}

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(i32)]
pub enum BorderStyle {
    #[default]
    None = 0,
    Hidden = 1,
    Dotted = 2,
    Dashed = 3,
    Solid = 4,
    Double = 5,
    Groove = 6,
    Ridge = 7,
    Inset = 8,
    Outset = 9,
}

impl BorderStyle {
    fn from_c_int(v: c_int) -> Self {
        match v {
            0 => Self::None,
            1 => Self::Hidden,
            2 => Self::Dotted,
            3 => Self::Dashed,
            4 => Self::Solid,
            5 => Self::Double,
            6 => Self::Groove,
            7 => Self::Ridge,
            8 => Self::Inset,
            9 => Self::Outset,
            _ => Self::None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(i32)]
pub enum MediaType {
    #[default]
    Unknown = 0,
    All = 1,
    Print = 2,
    Screen = 3,
}

impl MediaType {
    fn from_c_int(v: c_int) -> Self {
        match v {
            0 => Self::Unknown,
            1 => Self::All,
            2 => Self::Print,
            3 => Self::Screen,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(i32)]
pub enum TextTransform {
    #[default]
    None = 0,
    Capitalize = 1,
    Uppercase = 2,
    Lowercase = 3,
}

impl TextTransform {
    fn from_c_int(v: c_int) -> Self {
        match v {
            0 => Self::None,
            1 => Self::Capitalize,
            2 => Self::Uppercase,
            3 => Self::Lowercase,
            _ => Self::None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(i32)]
pub enum MouseEvent {
    #[default]
    Enter = 0,
    Leave = 1,
}

impl MouseEvent {
    fn from_c_int(v: c_int) -> Self {
        match v {
            0 => Self::Enter,
            1 => Self::Leave,
            _ => Self::Enter,
        }
    }
}

/// CSS gradient color interpolation space.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(i32)]
pub enum ColorSpace {
    #[default]
    None = 0,
    Srgb = 1,
    SrgbLinear = 2,
    DisplayP3 = 3,
    A98Rgb = 4,
    ProphotoRgb = 5,
    Rec2020 = 6,
    Lab = 7,
    Oklab = 8,
    Xyz = 9,
    XyzD50 = 10,
    XyzD65 = 11,
    Hsl = 12,
    Hwb = 13,
    Lch = 14,
    Oklch = 15,
}

impl ColorSpace {
    fn from_c_int(v: c_int) -> Self {
        match v {
            1 => Self::Srgb,
            2 => Self::SrgbLinear,
            3 => Self::DisplayP3,
            4 => Self::A98Rgb,
            5 => Self::ProphotoRgb,
            6 => Self::Rec2020,
            7 => Self::Lab,
            8 => Self::Oklab,
            9 => Self::Xyz,
            10 => Self::XyzD50,
            11 => Self::XyzD65,
            12 => Self::Hsl,
            13 => Self::Hwb,
            14 => Self::Lch,
            15 => Self::Oklch,
            _ => Self::None,
        }
    }
}

/// CSS gradient hue interpolation method.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(i32)]
pub enum HueInterpolation {
    #[default]
    None = 0,
    Shorter = 1,
    Longer = 2,
    Increasing = 3,
    Decreasing = 4,
}

impl HueInterpolation {
    fn from_c_int(v: c_int) -> Self {
        match v {
            1 => Self::Shorter,
            2 => Self::Longer,
            3 => Self::Increasing,
            4 => Self::Decreasing,
            _ => Self::None,
        }
    }
}

// ---------------------------------------------------------------------------
// Conversions: Rust types <-> C types
// ---------------------------------------------------------------------------

impl From<sys::lh_position_t> for Position {
    fn from(p: sys::lh_position_t) -> Self {
        Self {
            x: p.x,
            y: p.y,
            width: p.width,
            height: p.height,
        }
    }
}

impl From<Position> for sys::lh_position_t {
    fn from(p: Position) -> Self {
        Self {
            x: p.x,
            y: p.y,
            width: p.width,
            height: p.height,
        }
    }
}

impl From<sys::lh_size_t> for Size {
    fn from(s: sys::lh_size_t) -> Self {
        Self {
            width: s.width,
            height: s.height,
        }
    }
}

impl From<Size> for sys::lh_size_t {
    fn from(s: Size) -> Self {
        Self {
            width: s.width,
            height: s.height,
        }
    }
}

impl From<sys::lh_web_color_t> for Color {
    fn from(c: sys::lh_web_color_t) -> Self {
        Self {
            r: c.red,
            g: c.green,
            b: c.blue,
            a: c.alpha,
        }
    }
}

impl From<Color> for sys::lh_web_color_t {
    fn from(c: Color) -> Self {
        Self {
            red: c.r,
            green: c.g,
            blue: c.b,
            alpha: c.a,
            is_current_color: 0,
        }
    }
}

impl From<sys::lh_font_metrics_t> for FontMetrics {
    fn from(m: sys::lh_font_metrics_t) -> Self {
        Self {
            font_size: m.font_size,
            height: m.height,
            ascent: m.ascent,
            descent: m.descent,
            x_height: m.x_height,
            ch_width: m.ch_width,
            draw_spaces: m.draw_spaces != 0,
            sub_shift: m.sub_shift,
            super_shift: m.super_shift,
        }
    }
}

impl From<FontMetrics> for sys::lh_font_metrics_t {
    fn from(m: FontMetrics) -> Self {
        Self {
            font_size: m.font_size,
            height: m.height,
            ascent: m.ascent,
            descent: m.descent,
            x_height: m.x_height,
            ch_width: m.ch_width,
            draw_spaces: i32::from(m.draw_spaces),
            sub_shift: m.sub_shift,
            super_shift: m.super_shift,
        }
    }
}

impl From<sys::lh_border_radiuses_t> for BorderRadiuses {
    fn from(r: sys::lh_border_radiuses_t) -> Self {
        Self {
            top_left_x: r.top_left_x,
            top_left_y: r.top_left_y,
            top_right_x: r.top_right_x,
            top_right_y: r.top_right_y,
            bottom_right_x: r.bottom_right_x,
            bottom_right_y: r.bottom_right_y,
            bottom_left_x: r.bottom_left_x,
            bottom_left_y: r.bottom_left_y,
        }
    }
}

impl From<BorderRadiuses> for sys::lh_border_radiuses_t {
    fn from(r: BorderRadiuses) -> Self {
        Self {
            top_left_x: r.top_left_x,
            top_left_y: r.top_left_y,
            top_right_x: r.top_right_x,
            top_right_y: r.top_right_y,
            bottom_right_x: r.bottom_right_x,
            bottom_right_y: r.bottom_right_y,
            bottom_left_x: r.bottom_left_x,
            bottom_left_y: r.bottom_left_y,
        }
    }
}

impl From<sys::lh_border_t> for Border {
    fn from(b: sys::lh_border_t) -> Self {
        Self {
            width: b.width,
            style: BorderStyle::from_c_int(b.style),
            color: Color::from(b.color),
        }
    }
}

impl From<Border> for sys::lh_border_t {
    fn from(b: Border) -> Self {
        Self {
            width: b.width,
            style: b.style as c_int,
            color: b.color.into(),
        }
    }
}

impl From<sys::lh_borders_t> for Borders {
    fn from(b: sys::lh_borders_t) -> Self {
        Self {
            left: Border::from(b.left),
            top: Border::from(b.top),
            right: Border::from(b.right),
            bottom: Border::from(b.bottom),
            radius: BorderRadiuses::from(b.radius),
        }
    }
}

impl From<Borders> for sys::lh_borders_t {
    fn from(b: Borders) -> Self {
        Self {
            left: b.left.into(),
            top: b.top.into(),
            right: b.right.into(),
            bottom: b.bottom.into(),
            radius: b.radius.into(),
        }
    }
}

impl From<sys::lh_media_features_t> for MediaFeatures {
    fn from(m: sys::lh_media_features_t) -> Self {
        Self {
            media_type: MediaType::from_c_int(m.type_),
            width: m.width,
            height: m.height,
            device_width: m.device_width,
            device_height: m.device_height,
            color: m.color,
            color_index: m.color_index,
            monochrome: m.monochrome,
            resolution: m.resolution,
        }
    }
}

impl From<MediaFeatures> for sys::lh_media_features_t {
    fn from(m: MediaFeatures) -> Self {
        Self {
            type_: m.media_type as c_int,
            width: m.width,
            height: m.height,
            device_width: m.device_width,
            device_height: m.device_height,
            color: m.color,
            color_index: m.color_index,
            monochrome: m.monochrome,
            resolution: m.resolution,
        }
    }
}

impl From<sys::lh_point_t> for Point {
    fn from(p: sys::lh_point_t) -> Self {
        Self { x: p.x, y: p.y }
    }
}

impl From<Point> for sys::lh_point_t {
    fn from(p: Point) -> Self {
        Self { x: p.x, y: p.y }
    }
}

/// CSS `text-decoration-style` values.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TextDecorationStyle {
    #[default]
    Solid = 0,
    Double = 1,
    Dotted = 2,
    Dashed = 3,
    Wavy = 4,
}

impl TextDecorationStyle {
    fn from_c_int(v: c_int) -> Self {
        match v {
            1 => Self::Double,
            2 => Self::Dotted,
            3 => Self::Dashed,
            4 => Self::Wavy,
            _ => Self::Solid,
        }
    }
}

/// CSS `text-decoration-thickness` computed value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DecorationThickness {
    Auto,
    FromFont,
    Length(f32),
}

impl Default for DecorationThickness {
    fn default() -> Self {
        Self::Auto
    }
}

// ---------------------------------------------------------------------------
// Opaque pointer wrappers (borrowed, read-only)
// ---------------------------------------------------------------------------

/// Borrowed reference to a font description from litehtml.
pub struct FontDescription<'a> {
    ptr: *const sys::lh_font_description_t,
    _phantom: PhantomData<&'a ()>,
}

impl<'a> FontDescription<'a> {
    /// # Safety
    ///
    /// `ptr` must be a valid, non-null pointer to a `lh_font_description_t`
    /// that lives at least as long as `'a`.
    unsafe fn from_ptr(ptr: *const sys::lh_font_description_t) -> Self {
        Self {
            ptr,
            _phantom: PhantomData,
        }
    }

    pub fn family(&self) -> &'a str {
        unsafe {
            let ptr = sys::lh_font_description_family(self.ptr);
            if ptr.is_null() {
                ""
            } else {
                CStr::from_ptr(ptr).to_str().unwrap_or("")
            }
        }
    }

    pub fn size(&self) -> f32 {
        unsafe { sys::lh_font_description_size(self.ptr) }
    }

    pub fn style(&self) -> i32 {
        unsafe { sys::lh_font_description_style(self.ptr) }
    }

    pub fn weight(&self) -> i32 {
        unsafe { sys::lh_font_description_weight(self.ptr) }
    }

    pub fn decoration_line(&self) -> i32 {
        unsafe { sys::lh_font_description_decoration_line(self.ptr) }
    }

    pub fn decoration_thickness(&self) -> DecorationThickness {
        let is_predef =
            unsafe { sys::lh_font_description_decoration_thickness_is_predefined(self.ptr) };
        if is_predef != 0 {
            let predef =
                unsafe { sys::lh_font_description_decoration_thickness_predef(self.ptr) };
            match predef {
                1 => DecorationThickness::FromFont,
                _ => DecorationThickness::Auto,
            }
        } else {
            DecorationThickness::Length(unsafe {
                sys::lh_font_description_decoration_thickness_value(self.ptr)
            })
        }
    }

    pub fn decoration_style(&self) -> TextDecorationStyle {
        TextDecorationStyle::from_c_int(unsafe {
            sys::lh_font_description_decoration_style(self.ptr)
        })
    }

    pub fn decoration_color(&self) -> Color {
        Color::from(unsafe { sys::lh_font_description_decoration_color(self.ptr) })
    }

    pub fn emphasis_style(&self) -> &'a str {
        unsafe {
            let ptr = sys::lh_font_description_emphasis_style(self.ptr);
            if ptr.is_null() {
                ""
            } else {
                CStr::from_ptr(ptr).to_str().unwrap_or("")
            }
        }
    }

    pub fn emphasis_color(&self) -> Color {
        Color::from(unsafe { sys::lh_font_description_emphasis_color(self.ptr) })
    }

    pub fn emphasis_position(&self) -> i32 {
        unsafe { sys::lh_font_description_emphasis_position(self.ptr) }
    }
}

impl std::fmt::Debug for FontDescription<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FontDescription")
            .field("family", &self.family())
            .field("size", &self.size())
            .field("style", &self.style())
            .field("weight", &self.weight())
            .field("decoration_line", &self.decoration_line())
            .field("decoration_thickness", &self.decoration_thickness())
            .field("decoration_style", &self.decoration_style())
            .field("decoration_color", &self.decoration_color())
            .field("emphasis_style", &self.emphasis_style())
            .field("emphasis_color", &self.emphasis_color())
            .field("emphasis_position", &self.emphasis_position())
            .finish()
    }
}

/// Borrowed reference to a list marker from litehtml.
pub struct ListMarker<'a> {
    ptr: *const sys::lh_list_marker_t,
    _phantom: PhantomData<&'a ()>,
}

impl<'a> ListMarker<'a> {
    /// # Safety
    ///
    /// `ptr` must be a valid, non-null pointer that lives at least as long as `'a`.
    unsafe fn from_ptr(ptr: *const sys::lh_list_marker_t) -> Self {
        Self {
            ptr,
            _phantom: PhantomData,
        }
    }

    pub fn image(&self) -> &'a str {
        unsafe {
            let ptr = sys::lh_list_marker_image(self.ptr);
            if ptr.is_null() {
                ""
            } else {
                CStr::from_ptr(ptr).to_str().unwrap_or("")
            }
        }
    }

    pub fn baseurl(&self) -> &'a str {
        unsafe {
            let ptr = sys::lh_list_marker_baseurl(self.ptr);
            if ptr.is_null() {
                ""
            } else {
                CStr::from_ptr(ptr).to_str().unwrap_or("")
            }
        }
    }

    pub fn marker_type(&self) -> i32 {
        unsafe { sys::lh_list_marker_type(self.ptr) }
    }

    pub fn color(&self) -> Color {
        Color::from(unsafe { sys::lh_list_marker_color(self.ptr) })
    }

    pub fn pos(&self) -> Position {
        Position::from(unsafe { sys::lh_list_marker_pos(self.ptr) })
    }

    pub fn index(&self) -> i32 {
        unsafe { sys::lh_list_marker_index(self.ptr) }
    }

    pub fn font(&self) -> usize {
        unsafe { sys::lh_list_marker_font(self.ptr) }
    }
}

impl std::fmt::Debug for ListMarker<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ListMarker")
            .field("image", &self.image())
            .field("baseurl", &self.baseurl())
            .field("marker_type", &self.marker_type())
            .field("color", &self.color())
            .field("pos", &self.pos())
            .field("index", &self.index())
            .field("font", &self.font())
            .finish()
    }
}

/// Borrowed reference to a background layer from litehtml.
pub struct BackgroundLayer<'a> {
    ptr: *const sys::lh_background_layer_t,
    _phantom: PhantomData<&'a ()>,
}

impl<'a> BackgroundLayer<'a> {
    /// # Safety
    ///
    /// `ptr` must be a valid, non-null pointer that lives at least as long as `'a`.
    unsafe fn from_ptr(ptr: *const sys::lh_background_layer_t) -> Self {
        Self {
            ptr,
            _phantom: PhantomData,
        }
    }

    pub fn border_box(&self) -> Position {
        Position::from(unsafe { sys::lh_background_layer_border_box(self.ptr) })
    }

    pub fn border_radius(&self) -> BorderRadiuses {
        BorderRadiuses::from(unsafe { sys::lh_background_layer_border_radius(self.ptr) })
    }

    pub fn clip_box(&self) -> Position {
        Position::from(unsafe { sys::lh_background_layer_clip_box(self.ptr) })
    }

    pub fn origin_box(&self) -> Position {
        Position::from(unsafe { sys::lh_background_layer_origin_box(self.ptr) })
    }

    pub fn attachment(&self) -> i32 {
        unsafe { sys::lh_background_layer_attachment(self.ptr) }
    }

    pub fn repeat(&self) -> i32 {
        unsafe { sys::lh_background_layer_repeat(self.ptr) }
    }

    pub fn is_root(&self) -> bool {
        unsafe { sys::lh_background_layer_is_root(self.ptr) != 0 }
    }
}

impl std::fmt::Debug for BackgroundLayer<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BackgroundLayer")
            .field("border_box", &self.border_box())
            .field("clip_box", &self.clip_box())
            .field("origin_box", &self.origin_box())
            .field("is_root", &self.is_root())
            .finish()
    }
}

/// Borrowed reference to a linear gradient from litehtml.
pub struct LinearGradient<'a> {
    ptr: *const sys::lh_linear_gradient_t,
    _phantom: PhantomData<&'a ()>,
}

impl<'a> LinearGradient<'a> {
    /// # Safety
    ///
    /// `ptr` must be a valid, non-null pointer that lives at least as long as `'a`.
    unsafe fn from_ptr(ptr: *const sys::lh_linear_gradient_t) -> Self {
        Self {
            ptr,
            _phantom: PhantomData,
        }
    }

    pub fn start(&self) -> Point {
        Point::from(unsafe { sys::lh_linear_gradient_start(self.ptr) })
    }

    pub fn end(&self) -> Point {
        Point::from(unsafe { sys::lh_linear_gradient_end(self.ptr) })
    }

    pub fn color_points_count(&self) -> i32 {
        unsafe { sys::lh_linear_gradient_color_points_count(self.ptr) }
    }

    pub fn color_points(&self) -> Vec<ColorPoint> {
        let count = self.color_points_count();
        (0..count)
            .map(|i| ColorPoint {
                offset: unsafe { sys::lh_linear_gradient_color_point_offset(self.ptr, i) },
                color: Color::from(unsafe {
                    sys::lh_linear_gradient_color_point_color(self.ptr, i)
                }),
            })
            .collect()
    }

    pub fn color_space(&self) -> ColorSpace {
        ColorSpace::from_c_int(unsafe { sys::lh_linear_gradient_color_space(self.ptr) })
    }

    pub fn hue_interpolation(&self) -> HueInterpolation {
        HueInterpolation::from_c_int(unsafe {
            sys::lh_linear_gradient_hue_interpolation(self.ptr)
        })
    }
}

impl std::fmt::Debug for LinearGradient<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LinearGradient")
            .field("start", &self.start())
            .field("end", &self.end())
            .field("color_points", &self.color_points())
            .field("color_space", &self.color_space())
            .field("hue_interpolation", &self.hue_interpolation())
            .finish()
    }
}

/// Borrowed reference to a radial gradient from litehtml.
pub struct RadialGradient<'a> {
    ptr: *const sys::lh_radial_gradient_t,
    _phantom: PhantomData<&'a ()>,
}

impl<'a> RadialGradient<'a> {
    /// # Safety
    ///
    /// `ptr` must be a valid, non-null pointer that lives at least as long as `'a`.
    unsafe fn from_ptr(ptr: *const sys::lh_radial_gradient_t) -> Self {
        Self {
            ptr,
            _phantom: PhantomData,
        }
    }

    pub fn position(&self) -> Point {
        Point::from(unsafe { sys::lh_radial_gradient_position(self.ptr) })
    }

    pub fn radius(&self) -> Point {
        Point::from(unsafe { sys::lh_radial_gradient_radius(self.ptr) })
    }

    pub fn color_points_count(&self) -> i32 {
        unsafe { sys::lh_radial_gradient_color_points_count(self.ptr) }
    }

    pub fn color_points(&self) -> Vec<ColorPoint> {
        let count = self.color_points_count();
        (0..count)
            .map(|i| ColorPoint {
                offset: unsafe { sys::lh_radial_gradient_color_point_offset(self.ptr, i) },
                color: Color::from(unsafe {
                    sys::lh_radial_gradient_color_point_color(self.ptr, i)
                }),
            })
            .collect()
    }

    pub fn color_space(&self) -> ColorSpace {
        ColorSpace::from_c_int(unsafe { sys::lh_radial_gradient_color_space(self.ptr) })
    }

    pub fn hue_interpolation(&self) -> HueInterpolation {
        HueInterpolation::from_c_int(unsafe {
            sys::lh_radial_gradient_hue_interpolation(self.ptr)
        })
    }
}

impl std::fmt::Debug for RadialGradient<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RadialGradient")
            .field("position", &self.position())
            .field("radius", &self.radius())
            .field("color_points", &self.color_points())
            .field("color_space", &self.color_space())
            .field("hue_interpolation", &self.hue_interpolation())
            .finish()
    }
}

/// Borrowed reference to a conic gradient from litehtml.
pub struct ConicGradient<'a> {
    ptr: *const sys::lh_conic_gradient_t,
    _phantom: PhantomData<&'a ()>,
}

impl<'a> ConicGradient<'a> {
    /// # Safety
    ///
    /// `ptr` must be a valid, non-null pointer that lives at least as long as `'a`.
    unsafe fn from_ptr(ptr: *const sys::lh_conic_gradient_t) -> Self {
        Self {
            ptr,
            _phantom: PhantomData,
        }
    }

    pub fn position(&self) -> Point {
        Point::from(unsafe { sys::lh_conic_gradient_position(self.ptr) })
    }

    pub fn angle(&self) -> f32 {
        unsafe { sys::lh_conic_gradient_angle(self.ptr) }
    }

    pub fn radius(&self) -> f32 {
        unsafe { sys::lh_conic_gradient_radius(self.ptr) }
    }

    pub fn color_points_count(&self) -> i32 {
        unsafe { sys::lh_conic_gradient_color_points_count(self.ptr) }
    }

    pub fn color_points(&self) -> Vec<ColorPoint> {
        let count = self.color_points_count();
        (0..count)
            .map(|i| ColorPoint {
                offset: unsafe { sys::lh_conic_gradient_color_point_offset(self.ptr, i) },
                color: Color::from(unsafe {
                    sys::lh_conic_gradient_color_point_color(self.ptr, i)
                }),
            })
            .collect()
    }

    pub fn color_space(&self) -> ColorSpace {
        ColorSpace::from_c_int(unsafe { sys::lh_conic_gradient_color_space(self.ptr) })
    }

    pub fn hue_interpolation(&self) -> HueInterpolation {
        HueInterpolation::from_c_int(unsafe {
            sys::lh_conic_gradient_hue_interpolation(self.ptr)
        })
    }
}

impl std::fmt::Debug for ConicGradient<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConicGradient")
            .field("position", &self.position())
            .field("angle", &self.angle())
            .field("radius", &self.radius())
            .field("color_points", &self.color_points())
            .field("color_space", &self.color_space())
            .field("hue_interpolation", &self.hue_interpolation())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// DocumentContainer trait
// ---------------------------------------------------------------------------

/// Trait that must be implemented to provide drawing, font, and resource
/// callbacks to the litehtml rendering engine.
///
/// Methods with default implementations are optional overrides.
///
/// # Re-entrancy
///
/// litehtml calls these methods during [`Document::from_html`], [`Document::render`],
/// and [`Document::draw`]. Implementations must not call back into the [`Document`]
/// from within any trait method — doing so would create aliased mutable references
/// and is undefined behavior. The borrow checker enforces this for typical usage
/// since `Document` borrows the container mutably for its lifetime.
///
/// Implementations that use interior mutability (e.g., `RefCell`) should be aware
/// that litehtml calls multiple trait methods during a single `Document` operation
/// (e.g., `create_font` + `text_width` during `render`), but never re-entrantly
/// within a single method call.
#[allow(unused_variables)]
pub trait DocumentContainer {
    /// Create a font matching the given description. Returns a handle (used as
    /// an opaque identifier in subsequent calls) and the resulting metrics.
    fn create_font(&mut self, descr: &FontDescription) -> (usize, FontMetrics);

    /// Release resources associated with a previously created font handle.
    fn delete_font(&mut self, font: usize);

    /// Measure the width of `text` when rendered with `font`.
    fn text_width(&self, text: &str, font: usize) -> f32;

    /// Draw `text` at `pos` using `font` and `color`.
    fn draw_text(&mut self, hdc: usize, text: &str, font: usize, color: Color, pos: Position);

    /// Convert typographic points to device pixels.
    fn pt_to_px(&self, pt: f32) -> f32 {
        (pt * 96.0 / 72.0).round()
    }

    /// Default font size in pixels.
    fn default_font_size(&self) -> f32 {
        16.0
    }

    /// Default font family name.
    fn default_font_name(&self) -> &str {
        "serif"
    }

    /// Draw a list item marker (bullet, number, etc.).
    fn draw_list_marker(&mut self, hdc: usize, marker: &ListMarker) {}

    /// Notify the container that an image should be loaded.
    fn load_image(&mut self, src: &str, baseurl: &str, redraw_on_ready: bool);

    /// Return the intrinsic size of a previously loaded image.
    fn get_image_size(&self, src: &str, baseurl: &str) -> Size;

    /// Draw a background image layer.
    fn draw_image(&mut self, hdc: usize, layer: &BackgroundLayer, url: &str, base_url: &str);

    /// Draw a solid color background layer.
    fn draw_solid_fill(&mut self, hdc: usize, layer: &BackgroundLayer, color: Color);

    /// Draw a linear gradient background layer.
    fn draw_linear_gradient(
        &mut self,
        hdc: usize,
        layer: &BackgroundLayer,
        gradient: &LinearGradient,
    );

    /// Draw a radial gradient background layer.
    fn draw_radial_gradient(
        &mut self,
        hdc: usize,
        layer: &BackgroundLayer,
        gradient: &RadialGradient,
    );

    /// Draw a conic gradient background layer.
    fn draw_conic_gradient(
        &mut self,
        hdc: usize,
        layer: &BackgroundLayer,
        gradient: &ConicGradient,
    );

    /// Draw element borders.
    fn draw_borders(&mut self, hdc: usize, borders: &Borders, draw_pos: Position, root: bool) {}

    /// Set the document title.
    fn set_caption(&mut self, caption: &str) {}

    /// Set the document base URL.
    fn set_base_url(&mut self, base_url: &str) {}

    /// Called when a `<link>` element is encountered.
    fn link(&mut self) {}

    /// Called when the user clicks an anchor element.
    fn on_anchor_click(&mut self, url: &str) {}

    /// Called on mouse enter/leave events.
    fn on_mouse_event(&mut self, event: MouseEvent) {}

    /// Set the mouse cursor style.
    fn set_cursor(&mut self, cursor: &str) {}

    /// Transform text according to CSS `text-transform`. Returns the
    /// transformed string.
    fn transform_text(&self, text: &str, tt: TextTransform) -> String {
        text.to_string()
    }

    /// Import a CSS stylesheet from a URL. Returns the CSS content.
    fn import_css(&self, url: &str, baseurl: &str) -> String {
        String::new()
    }

    /// Push a clipping rectangle onto the clip stack.
    fn set_clip(&mut self, pos: Position, radius: BorderRadiuses) {}

    /// Pop the most recent clipping rectangle.
    fn del_clip(&mut self) {}

    /// Return the current viewport rectangle.
    fn get_viewport(&self) -> Position;

    /// Return the current media features for CSS media queries.
    fn get_media_features(&self) -> MediaFeatures;

    /// Return the document language and culture (e.g. `("en", "US")`).
    fn get_language(&self) -> (String, String) {
        ("en".to_string(), String::new())
    }
}

// ---------------------------------------------------------------------------
// FFI helpers
// ---------------------------------------------------------------------------

/// Safely convert a C string pointer to a `&str`, returning `""` for null
/// or non-UTF-8 input.
///
/// # Safety
///
/// If `ptr` is non-null it must point to a valid, null-terminated C string.
///
/// # Lifetime
///
/// The returned `&str` carries an unbounded lifetime `'a`. This is acceptable
/// for internal callback use where the C string is owned by litehtml and the
/// reference is consumed within the same callback frame. **Do not use this
/// function in public API methods** — prefer inlining the conversion with a
/// lifetime tied to `&self` or a struct lifetime parameter instead.
// SAFETY: all call sites are inside `extern "C"` callbacks where the C string
// is valid for the duration of the callback invocation. The resulting &str is
// passed directly to a DocumentContainer trait method and never escapes.
unsafe fn c_str_to_str<'a>(ptr: *const c_char) -> &'a str {
    if ptr.is_null() {
        ""
    } else {
        CStr::from_ptr(ptr).to_str().unwrap_or("")
    }
}

// ---------------------------------------------------------------------------
// Bridge: C callback functions that dispatch to DocumentContainer
// ---------------------------------------------------------------------------

/// Internal data kept alive for the duration of a [`Document`]. Stores the
/// container reference and any cached values needed by callbacks.
///
/// A raw pointer to this struct is passed as `user_data` through the C FFI.
/// Each callback recovers `&mut BridgeData` via [`bridge_from_user_data`].
/// This is sound only because litehtml never calls container methods
/// re-entrantly — at most one callback is active at any time.
struct BridgeData<'a> {
    container: &'a mut dyn DocumentContainer,
    /// Cached null-terminated default font name, kept alive so the pointer
    /// returned by `get_default_font_name` remains valid.
    default_font_name: CString,
}

/// Recover a `&mut BridgeData` from the raw `user_data` pointer passed by
/// litehtml into every callback.
///
/// # Safety
///
/// - `user_data` must be a pointer originally obtained from
///   `Box::into_raw(Box::new(BridgeData { ... }))`.
///
/// - **No re-entrancy**: the caller must guarantee that no other `&mut BridgeData`
///   derived from the same `user_data` pointer is live at the time of the call.
///   In practice this means `DocumentContainer` method implementations must never
///   call back into the `Document` (which would cause litehtml to invoke more
///   callbacks while the current one holds `&mut BridgeData`). Creating two
///   simultaneous `&mut` references is immediate undefined behavior.
///
///   This invariant is currently upheld by Rust's borrow checker: `Document`
///   holds `&'a mut dyn DocumentContainer` for its entire lifetime, so the
///   container cannot simultaneously access the document. litehtml's C++ engine
///   also dispatches container callbacks sequentially, never re-entrantly.
unsafe fn bridge_from_user_data<'a>(user_data: *mut c_void) -> &'a mut BridgeData<'a> {
    &mut *(user_data as *mut BridgeData<'a>)
}

// Each `extern "C"` function below matches a field of `lh_container_vtable_t`.
// They all follow the same pattern:
//   1. Recover BridgeData from user_data
//   2. Convert C types to Rust types
//   3. Call the trait method
//   4. Convert the result back to C types
//   5. The entire body is wrapped in catch_unwind to prevent panics from
//      unwinding across the FFI boundary (which is UB).
//
// Re-entrancy constraint: each callback creates a temporary `&mut BridgeData`
// (and thus `&mut dyn DocumentContainer`) from the raw pointer. This is sound
// only because litehtml dispatches callbacks sequentially — it never calls a
// second container method while a previous one is still executing. If that
// invariant were violated, two `&mut` references to the same data would exist
// simultaneously, which is UB. The `DocumentContainer` trait methods must not
// call back into the `Document` for the same reason.

unsafe extern "C" fn cb_create_font(
    user_data: *mut c_void,
    descr: *const sys::lh_font_description_t,
    fm: *mut sys::lh_font_metrics_t,
) -> usize {
    catch_unwind(AssertUnwindSafe(|| {
        let bridge = bridge_from_user_data(user_data);
        let font_descr = FontDescription::from_ptr(descr);
        let (handle, metrics) = bridge.container.create_font(&font_descr);
        if !fm.is_null() {
            *fm = metrics.into();
        }
        handle
    }))
    .unwrap_or(0)
}

unsafe extern "C" fn cb_delete_font(user_data: *mut c_void, h_font: usize) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let bridge = bridge_from_user_data(user_data);
        bridge.container.delete_font(h_font);
    }));
}

unsafe extern "C" fn cb_text_width(
    user_data: *mut c_void,
    text: *const c_char,
    h_font: usize,
) -> f32 {
    catch_unwind(AssertUnwindSafe(|| {
        let bridge = bridge_from_user_data(user_data);
        let text = c_str_to_str(text);
        bridge.container.text_width(text, h_font)
    }))
    .unwrap_or(0.0)
}

unsafe extern "C" fn cb_draw_text(
    user_data: *mut c_void,
    hdc: usize,
    text: *const c_char,
    h_font: usize,
    color: sys::lh_web_color_t,
    pos: sys::lh_position_t,
) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let bridge = bridge_from_user_data(user_data);
        let text = c_str_to_str(text);
        bridge
            .container
            .draw_text(hdc, text, h_font, Color::from(color), Position::from(pos));
    }));
}

unsafe extern "C" fn cb_pt_to_px(user_data: *mut c_void, pt: f32) -> f32 {
    catch_unwind(AssertUnwindSafe(|| {
        let bridge = bridge_from_user_data(user_data);
        bridge.container.pt_to_px(pt)
    }))
    .unwrap_or(0.0)
}

unsafe extern "C" fn cb_get_default_font_size(user_data: *mut c_void) -> f32 {
    catch_unwind(AssertUnwindSafe(|| {
        let bridge = bridge_from_user_data(user_data);
        bridge.container.default_font_size()
    }))
    .unwrap_or(16.0)
}

unsafe extern "C" fn cb_get_default_font_name(user_data: *mut c_void) -> *const c_char {
    catch_unwind(AssertUnwindSafe(|| {
        let bridge = bridge_from_user_data(user_data);
        bridge.default_font_name.as_ptr()
    }))
    .unwrap_or(b"serif\0".as_ptr().cast::<c_char>())
}

unsafe extern "C" fn cb_draw_list_marker(
    user_data: *mut c_void,
    hdc: usize,
    marker: *const sys::lh_list_marker_t,
) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let bridge = bridge_from_user_data(user_data);
        let marker = ListMarker::from_ptr(marker);
        bridge.container.draw_list_marker(hdc, &marker);
    }));
}

unsafe extern "C" fn cb_load_image(
    user_data: *mut c_void,
    src: *const c_char,
    baseurl: *const c_char,
    redraw_on_ready: c_int,
) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let bridge = bridge_from_user_data(user_data);
        let src = c_str_to_str(src);
        let baseurl = c_str_to_str(baseurl);
        bridge
            .container
            .load_image(src, baseurl, redraw_on_ready != 0);
    }));
}

unsafe extern "C" fn cb_get_image_size(
    user_data: *mut c_void,
    src: *const c_char,
    baseurl: *const c_char,
    sz: *mut sys::lh_size_t,
) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let bridge = bridge_from_user_data(user_data);
        let src = c_str_to_str(src);
        let baseurl = c_str_to_str(baseurl);
        let size = bridge.container.get_image_size(src, baseurl);
        if !sz.is_null() {
            *sz = size.into();
        }
    }));
}

unsafe extern "C" fn cb_draw_image(
    user_data: *mut c_void,
    hdc: usize,
    layer: *const sys::lh_background_layer_t,
    url: *const c_char,
    base_url: *const c_char,
) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let bridge = bridge_from_user_data(user_data);
        let layer = BackgroundLayer::from_ptr(layer);
        let url = c_str_to_str(url);
        let base_url = c_str_to_str(base_url);
        bridge.container.draw_image(hdc, &layer, url, base_url);
    }));
}

unsafe extern "C" fn cb_draw_solid_fill(
    user_data: *mut c_void,
    hdc: usize,
    layer: *const sys::lh_background_layer_t,
    color: sys::lh_web_color_t,
) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let bridge = bridge_from_user_data(user_data);
        let layer = BackgroundLayer::from_ptr(layer);
        bridge
            .container
            .draw_solid_fill(hdc, &layer, Color::from(color));
    }));
}

unsafe extern "C" fn cb_draw_linear_gradient(
    user_data: *mut c_void,
    hdc: usize,
    layer: *const sys::lh_background_layer_t,
    gradient: *const sys::lh_linear_gradient_t,
) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let bridge = bridge_from_user_data(user_data);
        let layer = BackgroundLayer::from_ptr(layer);
        let gradient = LinearGradient::from_ptr(gradient);
        bridge
            .container
            .draw_linear_gradient(hdc, &layer, &gradient);
    }));
}

unsafe extern "C" fn cb_draw_radial_gradient(
    user_data: *mut c_void,
    hdc: usize,
    layer: *const sys::lh_background_layer_t,
    gradient: *const sys::lh_radial_gradient_t,
) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let bridge = bridge_from_user_data(user_data);
        let layer = BackgroundLayer::from_ptr(layer);
        let gradient = RadialGradient::from_ptr(gradient);
        bridge
            .container
            .draw_radial_gradient(hdc, &layer, &gradient);
    }));
}

unsafe extern "C" fn cb_draw_conic_gradient(
    user_data: *mut c_void,
    hdc: usize,
    layer: *const sys::lh_background_layer_t,
    gradient: *const sys::lh_conic_gradient_t,
) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let bridge = bridge_from_user_data(user_data);
        let layer = BackgroundLayer::from_ptr(layer);
        let gradient = ConicGradient::from_ptr(gradient);
        bridge.container.draw_conic_gradient(hdc, &layer, &gradient);
    }));
}

unsafe extern "C" fn cb_draw_borders(
    user_data: *mut c_void,
    hdc: usize,
    borders: sys::lh_borders_t,
    draw_pos: sys::lh_position_t,
    root: c_int,
) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let bridge = bridge_from_user_data(user_data);
        let borders = Borders::from(borders);
        bridge
            .container
            .draw_borders(hdc, &borders, Position::from(draw_pos), root != 0);
    }));
}

unsafe extern "C" fn cb_set_caption(user_data: *mut c_void, caption: *const c_char) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let bridge = bridge_from_user_data(user_data);
        let caption = c_str_to_str(caption);
        bridge.container.set_caption(caption);
    }));
}

unsafe extern "C" fn cb_set_base_url(user_data: *mut c_void, base_url: *const c_char) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let bridge = bridge_from_user_data(user_data);
        let base_url = c_str_to_str(base_url);
        bridge.container.set_base_url(base_url);
    }));
}

unsafe extern "C" fn cb_link(user_data: *mut c_void) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let bridge = bridge_from_user_data(user_data);
        bridge.container.link();
    }));
}

unsafe extern "C" fn cb_on_anchor_click(user_data: *mut c_void, url: *const c_char) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let bridge = bridge_from_user_data(user_data);
        let url = c_str_to_str(url);
        bridge.container.on_anchor_click(url);
    }));
}

unsafe extern "C" fn cb_on_mouse_event(user_data: *mut c_void, event: c_int) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let bridge = bridge_from_user_data(user_data);
        bridge
            .container
            .on_mouse_event(MouseEvent::from_c_int(event));
    }));
}

unsafe extern "C" fn cb_set_cursor(user_data: *mut c_void, cursor: *const c_char) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let bridge = bridge_from_user_data(user_data);
        let cursor = c_str_to_str(cursor);
        bridge.container.set_cursor(cursor);
    }));
}

unsafe extern "C" fn cb_transform_text(
    user_data: *mut c_void,
    text: *const c_char,
    tt: c_int,
    set_result: sys::lh_set_string_fn,
    ctx: *mut c_void,
) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let bridge = bridge_from_user_data(user_data);
        let text = c_str_to_str(text);
        let transform = TextTransform::from_c_int(tt);
        let result = bridge.container.transform_text(text, transform);
        if let Some(set_fn) = set_result {
            let c_result = CString::new(result).unwrap_or_else(|_| {
                warn!("transform_text result contained interior null byte, using empty string");
                CString::default()
            });
            set_fn(ctx, c_result.as_ptr());
        }
    }));
}

unsafe extern "C" fn cb_import_css(
    user_data: *mut c_void,
    url: *const c_char,
    baseurl: *const c_char,
    set_result: sys::lh_set_string_fn,
    ctx: *mut c_void,
) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let bridge = bridge_from_user_data(user_data);
        let url = c_str_to_str(url);
        let baseurl = c_str_to_str(baseurl);
        let result = bridge.container.import_css(url, baseurl);
        if let Some(set_fn) = set_result {
            let c_result = CString::new(result).unwrap_or_else(|_| {
                warn!("import_css result contained interior null byte, using empty string");
                CString::default()
            });
            set_fn(ctx, c_result.as_ptr());
        }
    }));
}

unsafe extern "C" fn cb_set_clip(
    user_data: *mut c_void,
    pos: sys::lh_position_t,
    bdr_radius: sys::lh_border_radiuses_t,
) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let bridge = bridge_from_user_data(user_data);
        bridge
            .container
            .set_clip(Position::from(pos), BorderRadiuses::from(bdr_radius));
    }));
}

unsafe extern "C" fn cb_del_clip(user_data: *mut c_void) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let bridge = bridge_from_user_data(user_data);
        bridge.container.del_clip();
    }));
}

unsafe extern "C" fn cb_get_viewport(user_data: *mut c_void, viewport: *mut sys::lh_position_t) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let bridge = bridge_from_user_data(user_data);
        let vp = bridge.container.get_viewport();
        if !viewport.is_null() {
            *viewport = vp.into();
        }
    }));
}

unsafe extern "C" fn cb_get_media_features(
    user_data: *mut c_void,
    media: *mut sys::lh_media_features_t,
) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let bridge = bridge_from_user_data(user_data);
        let features = bridge.container.get_media_features();
        if !media.is_null() {
            *media = features.into();
        }
    }));
}

unsafe extern "C" fn cb_get_language(
    user_data: *mut c_void,
    set_result: sys::lh_set_language_fn,
    ctx: *mut c_void,
) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let bridge = bridge_from_user_data(user_data);
        let (lang, culture) = bridge.container.get_language();
        if let Some(set_fn) = set_result {
            let c_lang = CString::new(lang).unwrap_or_default();
            let c_culture = CString::new(culture).unwrap_or_default();
            set_fn(ctx, c_lang.as_ptr(), c_culture.as_ptr());
        }
    }));
}

/// Single shared vtable for all Document instances. Every field is a
/// compile-time-constant function pointer; per-document state is carried
/// through `user_data`, not the vtable.
static CONTAINER_VTABLE: sys::lh_container_vtable_t = sys::lh_container_vtable_t {
    create_font: Some(cb_create_font),
    delete_font: Some(cb_delete_font),
    text_width: Some(cb_text_width),
    draw_text: Some(cb_draw_text),
    pt_to_px: Some(cb_pt_to_px),
    get_default_font_size: Some(cb_get_default_font_size),
    get_default_font_name: Some(cb_get_default_font_name),
    draw_list_marker: Some(cb_draw_list_marker),
    load_image: Some(cb_load_image),
    get_image_size: Some(cb_get_image_size),
    draw_image: Some(cb_draw_image),
    draw_solid_fill: Some(cb_draw_solid_fill),
    draw_linear_gradient: Some(cb_draw_linear_gradient),
    draw_radial_gradient: Some(cb_draw_radial_gradient),
    draw_conic_gradient: Some(cb_draw_conic_gradient),
    draw_borders: Some(cb_draw_borders),
    set_caption: Some(cb_set_caption),
    set_base_url: Some(cb_set_base_url),
    link: Some(cb_link),
    on_anchor_click: Some(cb_on_anchor_click),
    on_mouse_event: Some(cb_on_mouse_event),
    set_cursor: Some(cb_set_cursor),
    transform_text: Some(cb_transform_text),
    import_css: Some(cb_import_css),
    set_clip: Some(cb_set_clip),
    del_clip: Some(cb_del_clip),
    get_viewport: Some(cb_get_viewport),
    get_media_features: Some(cb_get_media_features),
    get_language: Some(cb_get_language),
};

// ---------------------------------------------------------------------------
// Document
// ---------------------------------------------------------------------------

/// Opaque handle to a litehtml element. Borrows from the parent [`Document`].
pub struct Element<'a> {
    ptr: *mut sys::lh_element_t,
    _phantom: PhantomData<&'a ()>,
}

/// A parsed HTML document. Wraps the C++ `litehtml::document` and ties its
/// lifetime to the [`DocumentContainer`] that provides rendering callbacks.
///
/// `Document` is intentionally `!Send` and `!Sync` because the underlying
/// C++ engine is single-threaded.
///
/// ```compile_fail
/// fn assert_send<T: Send>() {}
/// assert_send::<litehtml::Document<'static>>();
/// ```
///
/// ```compile_fail
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<litehtml::Document<'static>>();
/// ```
pub struct Document<'a> {
    raw: *mut sys::lh_document_t,
    /// Kept alive so the user_data pointer inside litehtml remains valid.
    bridge: *mut BridgeData<'a>,
}

impl<'a> Document<'a> {
    /// Parse HTML into a document, using `container` for all rendering
    /// callbacks.
    ///
    /// Returns an error if the input strings contain interior null bytes
    /// or if litehtml fails to create the document.
    ///
    /// `master_css` is the user-agent stylesheet applied before any document
    /// styles. `user_styles` are additional CSS rules applied after the
    /// document styles.
    #[must_use = "document must be stored for rendering"]
    pub fn from_html(
        html: &str,
        container: &'a mut dyn DocumentContainer,
        master_css: Option<&str>,
        user_styles: Option<&str>,
    ) -> Result<Self, CreateError> {
        let c_html = CString::new(html)?;

        let c_master_css = master_css.map(CString::new).transpose()?;
        let c_user_styles = user_styles.map(CString::new).transpose()?;

        let master_css_ptr = c_master_css
            .as_ref()
            .map_or(std::ptr::null(), |s| s.as_ptr());
        let user_styles_ptr = c_user_styles
            .as_ref()
            .map_or(std::ptr::null(), |s| s.as_ptr());

        // Cache the default font name as a CString so get_default_font_name
        // can return a pointer that lives as long as the document.
        let font_name = container.default_font_name();
        let default_font_name =
            CString::new(font_name).unwrap_or_else(|_| CString::new("serif").unwrap());

        let bridge_data = BridgeData {
            container,
            default_font_name,
        };
        let bridge_ptr = Box::into_raw(Box::new(bridge_data));

        // SAFETY: the C++ side only reads through this pointer (never writes).
        // See CDocumentContainer in litehtml_c.cpp — all vtable access is read-only.
        let vtable_ptr =
            std::ptr::addr_of!(CONTAINER_VTABLE) as *mut sys::lh_container_vtable_t;

        let raw = unsafe {
            sys::lh_document_create_from_string(
                c_html.as_ptr(),
                vtable_ptr,
                bridge_ptr as *mut c_void,
                master_css_ptr,
                user_styles_ptr,
            )
        };

        if raw.is_null() {
            unsafe {
                drop(Box::from_raw(bridge_ptr));
            }
            return Err(CreateError::CreateFailed);
        }

        Ok(Self {
            raw,
            bridge: bridge_ptr,
        })
    }

    /// Lay out the document within `max_width` pixels. Returns the actual
    /// content width after layout.
    #[must_use = "returns the content width after layout"]
    pub fn render(&mut self, max_width: f32) -> f32 {
        unsafe { sys::lh_document_render(self.raw, max_width) }
    }

    /// Draw the document into the rendering context identified by `hdc`,
    /// at offset `(x, y)`. If `clip` is `Some`, only the intersection with
    /// the clip rectangle is drawn.
    pub fn draw(&mut self, hdc: usize, x: f32, y: f32, clip: Option<Position>) {
        let clip_c = clip.map(sys::lh_position_t::from);
        let clip_ptr = clip_c.as_ref().map_or(std::ptr::null(), |c| c as *const _);
        unsafe {
            sys::lh_document_draw(self.raw, hdc, x, y, clip_ptr);
        }
    }

    /// Content width after the most recent `render` call.
    pub fn width(&self) -> f32 {
        unsafe { sys::lh_document_width(self.raw) }
    }

    /// Content height after the most recent `render` call.
    pub fn height(&self) -> f32 {
        unsafe { sys::lh_document_height(self.raw) }
    }

    /// Notify the document of a mouse-move event. Returns `true` if the
    /// cursor or element states changed (i.e. a redraw is needed).
    pub fn on_mouse_over(&mut self, x: f32, y: f32, client_x: f32, client_y: f32) -> bool {
        unsafe { sys::lh_document_on_mouse_over(self.raw, x, y, client_x, client_y) != 0 }
    }

    /// Notify the document of a left-button-down event. Returns `true` if
    /// a redraw is needed.
    pub fn on_lbutton_down(&mut self, x: f32, y: f32, client_x: f32, client_y: f32) -> bool {
        unsafe { sys::lh_document_on_lbutton_down(self.raw, x, y, client_x, client_y) != 0 }
    }

    /// Notify the document of a left-button-up event. Returns `true` if
    /// a redraw is needed.
    pub fn on_lbutton_up(&mut self, x: f32, y: f32, client_x: f32, client_y: f32) -> bool {
        unsafe { sys::lh_document_on_lbutton_up(self.raw, x, y, client_x, client_y) != 0 }
    }

    /// Notify the document that the mouse has left its area. Returns `true`
    /// if a redraw is needed.
    pub fn on_mouse_leave(&mut self) -> bool {
        unsafe { sys::lh_document_on_mouse_leave(self.raw) != 0 }
    }

    /// Re-evaluate CSS media queries after the viewport or media features
    /// have changed. Returns `true` if styles changed and a re-render is
    /// needed.
    pub fn media_changed(&mut self) -> bool {
        unsafe { sys::lh_document_media_changed(self.raw) != 0 }
    }

    /// Add a CSS stylesheet to the document and apply it immediately.
    ///
    /// This parses the CSS, applies matching rules to all elements, and
    /// recomputes styles. A subsequent [`render`](Self::render) call is
    /// needed to update the layout.
    pub fn add_stylesheet(
        &mut self,
        css: &str,
        baseurl: Option<&str>,
        media: Option<&str>,
    ) -> Result<(), CreateError> {
        let c_css = CString::new(css)?;
        let c_baseurl = baseurl.map(CString::new).transpose()?;
        let c_media = media.map(CString::new).transpose()?;
        let baseurl_ptr = c_baseurl
            .as_ref()
            .map_or(std::ptr::null(), |s| s.as_ptr());
        let media_ptr = c_media.as_ref().map_or(std::ptr::null(), |s| s.as_ptr());
        unsafe {
            sys::lh_document_add_stylesheet(self.raw, c_css.as_ptr(), baseurl_ptr, media_ptr);
        }
        Ok(())
    }

    /// Get the root element of the document.
    pub fn root(&self) -> Option<Element<'_>> {
        let ptr = unsafe { sys::lh_document_root(self.raw) };
        if ptr.is_null() {
            None
        } else {
            Some(Element {
                ptr,
                _phantom: PhantomData,
            })
        }
    }

    /// Parse an HTML fragment and append the resulting elements as children
    /// of `parent`.
    ///
    /// If `replace_existing` is true, all existing children of `parent` are
    /// removed first. A subsequent [`render`](Self::render) call is needed
    /// to update the layout.
    pub fn append_children_from_string(
        &mut self,
        parent: &Element<'_>,
        html: &str,
        replace_existing: bool,
    ) -> Result<(), CreateError> {
        let c_html = CString::new(html)?;
        unsafe {
            sys::lh_document_append_children_from_string(
                self.raw,
                parent.ptr,
                c_html.as_ptr(),
                if replace_existing { 1 } else { 0 },
            );
        }
        Ok(())
    }
}

impl Drop for Document<'_> {
    fn drop(&mut self) {
        unsafe {
            // Destroy the document first (it may call callbacks during teardown),
            // then free the bridge data.
            sys::lh_document_destroy(self.raw);
            drop(Box::from_raw(self.bridge));
        }
    }
}

// ---------------------------------------------------------------------------
// Optional pixbuf rendering backend
// ---------------------------------------------------------------------------

#[cfg(feature = "pixbuf")]
pub mod pixbuf;

#[cfg(feature = "email")]
pub mod email;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal container that stubs every required method with safe defaults.
    struct TestContainer {
        next_font_id: usize,
    }

    impl TestContainer {
        fn new() -> Self {
            Self { next_font_id: 1 }
        }
    }

    impl DocumentContainer for TestContainer {
        fn create_font(&mut self, _descr: &FontDescription) -> (usize, FontMetrics) {
            let id = self.next_font_id;
            self.next_font_id += 1;
            let metrics = FontMetrics {
                font_size: 16.0,
                height: 20.0,
                ascent: 16.0,
                descent: 4.0,
                x_height: 8.0,
                ch_width: 8.0,
                draw_spaces: false,
                sub_shift: 0.0,
                super_shift: 0.0,
            };
            (id, metrics)
        }

        fn delete_font(&mut self, _font: usize) {}

        fn text_width(&self, text: &str, _font: usize) -> f32 {
            // Rough approximation: 8 px per character
            text.len() as f32 * 8.0
        }

        fn draw_text(
            &mut self,
            _hdc: usize,
            _text: &str,
            _font: usize,
            _color: Color,
            _pos: Position,
        ) {
        }

        fn draw_list_marker(&mut self, _hdc: usize, _marker: &ListMarker) {}

        fn load_image(&mut self, _src: &str, _baseurl: &str, _redraw_on_ready: bool) {}

        fn get_image_size(&self, _src: &str, _baseurl: &str) -> Size {
            Size::default()
        }

        fn draw_image(
            &mut self,
            _hdc: usize,
            _layer: &BackgroundLayer,
            _url: &str,
            _base_url: &str,
        ) {
        }

        fn draw_solid_fill(&mut self, _hdc: usize, _layer: &BackgroundLayer, _color: Color) {}

        fn draw_linear_gradient(
            &mut self,
            _hdc: usize,
            _layer: &BackgroundLayer,
            _gradient: &LinearGradient,
        ) {
        }

        fn draw_radial_gradient(
            &mut self,
            _hdc: usize,
            _layer: &BackgroundLayer,
            _gradient: &RadialGradient,
        ) {
        }

        fn draw_conic_gradient(
            &mut self,
            _hdc: usize,
            _layer: &BackgroundLayer,
            _gradient: &ConicGradient,
        ) {
        }

        fn draw_borders(
            &mut self,
            _hdc: usize,
            _borders: &Borders,
            _draw_pos: Position,
            _root: bool,
        ) {
        }

        fn set_caption(&mut self, _caption: &str) {}

        fn set_base_url(&mut self, _base_url: &str) {}

        fn on_anchor_click(&mut self, _url: &str) {}

        fn set_cursor(&mut self, _cursor: &str) {}

        fn set_clip(&mut self, _pos: Position, _radius: BorderRadiuses) {}

        fn del_clip(&mut self) {}

        fn get_viewport(&self) -> Position {
            Position {
                x: 0.0,
                y: 0.0,
                width: 800.0,
                height: 600.0,
            }
        }

        fn get_media_features(&self) -> MediaFeatures {
            MediaFeatures {
                media_type: MediaType::Screen,
                width: 800.0,
                height: 600.0,
                device_width: 800.0,
                device_height: 600.0,
                color: 8,
                color_index: 0,
                monochrome: 0,
                resolution: 96.0,
            }
        }
    }

    #[test]
    fn test_parse_simple_html() {
        let mut container = TestContainer::new();
        let doc = Document::from_html("<p>Hello</p>", &mut container, None, None);
        assert!(doc.is_ok());
    }

    #[test]
    fn test_render_dimensions() {
        let mut container = TestContainer::new();
        let mut doc =
            Document::from_html("<p>Hello world</p>", &mut container, None, None).unwrap();
        let _ = doc.render(800.0);
        assert!(doc.width() > 0.0);
        assert!(doc.height() > 0.0);
    }

    #[test]
    fn test_render_and_draw() {
        let mut container = TestContainer::new();
        let mut doc =
            Document::from_html("<h1>Title</h1><p>Body text</p>", &mut container, None, None)
                .unwrap();
        let _ = doc.render(800.0);
        let clip = Position {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 600.0,
        };
        doc.draw(0, 0.0, 0.0, Some(clip));
    }

    #[test]
    fn test_mouse_events() {
        let mut container = TestContainer::new();
        let mut doc =
            Document::from_html("<a href=\"#\">Link</a>", &mut container, None, None).unwrap();
        let _ = doc.render(800.0);
        let _ = doc.on_mouse_over(10.0, 10.0, 10.0, 10.0);
        let _ = doc.on_lbutton_down(10.0, 10.0, 10.0, 10.0);
        let _ = doc.on_lbutton_up(10.0, 10.0, 10.0, 10.0);
        let _ = doc.on_mouse_leave();
    }

    #[test]
    fn test_media_changed() {
        let mut container = TestContainer::new();
        let mut doc = Document::from_html("<p>Test</p>", &mut container, None, None).unwrap();
        let _ = doc.render(800.0);
        let _ = doc.media_changed();
    }

    #[test]
    fn test_with_master_css() {
        let mut container = TestContainer::new();
        let css = "body { margin: 0; padding: 0; }";
        let doc = Document::from_html("<p>Styled</p>", &mut container, Some(css), None);
        assert!(doc.is_ok());
    }

    #[test]
    fn test_with_user_styles() {
        let mut container = TestContainer::new();
        let user_css = "p { color: red; }";
        let doc = Document::from_html("<p>Red</p>", &mut container, None, Some(user_css));
        assert!(doc.is_ok());
    }

    #[test]
    fn test_color_default() {
        let c = Color::default();
        assert_eq!(c.r, 0);
        assert_eq!(c.g, 0);
        assert_eq!(c.b, 0);
        assert_eq!(c.a, 255);
    }

    #[test]
    fn test_color_roundtrip() {
        let c = Color {
            r: 255,
            g: 128,
            b: 64,
            a: 200,
        };
        let c_color: sys::lh_web_color_t = c.into();
        let back = Color::from(c_color);
        assert_eq!(c, back);
    }

    #[test]
    fn test_position_roundtrip() {
        let p = Position {
            x: 1.0,
            y: 2.0,
            width: 3.0,
            height: 4.0,
        };
        let c_pos: sys::lh_position_t = p.into();
        let back = Position::from(c_pos);
        assert_eq!(p, back);
    }

    #[test]
    fn test_border_style_from_c_int() {
        assert_eq!(BorderStyle::from_c_int(0), BorderStyle::None);
        assert_eq!(BorderStyle::from_c_int(4), BorderStyle::Solid);
        assert_eq!(BorderStyle::from_c_int(9), BorderStyle::Outset);
        assert_eq!(BorderStyle::from_c_int(99), BorderStyle::None);
    }

    #[test]
    fn test_media_type_from_c_int() {
        assert_eq!(MediaType::from_c_int(0), MediaType::Unknown);
        assert_eq!(MediaType::from_c_int(3), MediaType::Screen);
        assert_eq!(MediaType::from_c_int(42), MediaType::Unknown);
    }

    #[test]
    fn test_font_metrics_roundtrip() {
        let m = FontMetrics {
            font_size: 16.0,
            height: 20.0,
            ascent: 14.0,
            descent: 6.0,
            x_height: 8.0,
            ch_width: 8.0,
            draw_spaces: true,
            sub_shift: 2.0,
            super_shift: 3.0,
        };
        let c_m: sys::lh_font_metrics_t = m.into();
        let back = FontMetrics::from(c_m);
        assert_eq!(m, back);
    }

    // test_not_send_sync is now a compile_fail doc test on the Document struct.

    #[cfg(feature = "pixbuf")]
    mod pixbuf_tests {
        #[test]
        fn test_render_to_rgba_produces_pixels() {
            let html = r#"<body style="background: white;"><p style="color: black;">Hello world</p></body>"#;
            let pixels = crate::pixbuf::render_to_rgba(html, 200, 100);
            assert_eq!(pixels.len(), 200 * 100 * 4);
            // At least some pixels should be non-transparent (white background)
            let has_opaque = pixels.chunks(4).any(|px| px[3] > 0);
            assert!(has_opaque, "render should produce non-transparent pixels");
        }

        #[test]
        fn test_pixbuf_container_resize() {
            let mut c = crate::pixbuf::PixbufContainer::new(100, 100);
            assert_eq!(c.width(), 100);
            assert_eq!(c.height(), 100);
            c.resize(200, 150);
            assert_eq!(c.width(), 200);
            assert_eq!(c.height(), 150);
        }

        #[test]
        fn test_pixbuf_render_with_borders() {
            let html = r#"<div style="border: 2px solid red; width: 50px; height: 50px; background: blue;"></div>"#;
            let pixels = crate::pixbuf::render_to_rgba(html, 200, 200);
            assert_eq!(pixels.len(), 200 * 200 * 4);
        }

        // -- Integration tests for real-world email HTML patterns --
        // These require both the "pixbuf" and "email" features.

        #[cfg(feature = "email")]
        #[test]
        fn test_email_render_mailchimp_style() {
            let html = br##"<html>
<head>
    <meta charset="utf-8">
</head>
<body bgcolor="#f2f2f2" style="margin: 0; padding: 0;">
<table width="100%" cellpadding="0" cellspacing="0" border="0">
<tr><td align="center">
    <table width="600" cellpadding="10" cellspacing="0" border="0" style="background-color: #ffffff;">
    <tr>
        <td style="padding: 20px;">
            <img src="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==" width="100" height="20" alt="Logo">
        </td>
    </tr>
    <tr>
        <td>
            <table width="100%" cellpadding="5" cellspacing="0">
            <tr>
                <td width="50%" style="vertical-align: top; padding: 10px;">
                    <font face="Arial, sans-serif" size="4" color="#333333"><b>Left Column</b></font>
                    <p style="font-family: Arial, sans-serif; font-size: 14px; color: #666666;">
                        Marketing content goes here with <a href="#" style="color: #007bff; text-decoration: underline;">a styled link</a>.
                    </p>
                </td>
                <td width="50%" style="vertical-align: top; padding: 10px;">
                    <font face="Arial, sans-serif" size="4" color="#333333"><b>Right Column</b></font>
                    <p style="font-family: Arial, sans-serif; font-size: 14px; color: #666666;">
                        Second column content with more text to fill the space.
                    </p>
                </td>
            </tr>
            </table>
        </td>
    </tr>
    <tr>
        <td style="background-color: #007bff; text-align: center; padding: 15px;">
            <a href="#" style="color: #ffffff; font-family: Arial, sans-serif; font-size: 16px; text-decoration: none; font-weight: bold;">Call to Action</a>
        </td>
    </tr>
    </table>
</td></tr>
</table>
</body>
</html>"##;

            let prepared = crate::email::prepare_email_html(html, None, None);

            // Sanitization: no dangerous elements should exist
            assert!(
                !prepared.html.contains("<script"),
                "script tags must be stripped"
            );
            assert!(
                !prepared.html.contains("onclick"),
                "event handlers must be stripped"
            );

            // data: URI image should have been resolved
            assert!(
                !prepared.images.is_empty(),
                "data: URI image should be extracted"
            );

            let pixels = crate::pixbuf::render_to_rgba(&prepared.html, 600, 800);
            assert_eq!(pixels.len(), 600 * 800 * 4, "pixel buffer size mismatch");

            let has_opaque = pixels.chunks(4).any(|px| px[3] > 0);
            assert!(has_opaque, "render should produce non-transparent pixels");

            let has_non_white = pixels
                .chunks(4)
                .any(|px| px[3] > 0 && (px[0] < 250 || px[1] < 250 || px[2] < 250));
            assert!(
                has_non_white,
                "render should contain non-white pixels from actual content"
            );
        }

        #[cfg(feature = "email")]
        #[test]
        fn test_email_render_gmail_style() {
            let html = br##"<html>
<head>
    <meta charset="utf-8">
    <style>
        .email-body { font-family: Roboto, Arial, sans-serif; color: #202124; }
        .header { font-size: 24px; font-weight: bold; margin-bottom: 16px; }
        .content { font-size: 14px; line-height: 1.5; }
        .footer { color: #5f6368; font-size: 12px; margin-top: 20px; }
    </style>
</head>
<body>
<div class="email-body" style="max-width: 600px; margin: 0 auto; padding: 20px;">
    <div class="header">Welcome to the Service</div>
    <div class="content">
        <p>Hi there,</p>
        <p>Thanks for signing up. Here is your confirmation details:</p>
        <table style="width: 100%; margin: 16px 0;">
            <tr>
                <td style="background-color: #f8f9fa; padding: 12px; border: 1px solid #dadce0;">
                    <strong>Account</strong>
                </td>
                <td style="background-color: #f8f9fa; padding: 12px; border: 1px solid #dadce0;">
                    user@example.com
                </td>
            </tr>
        </table>
        <h2 style="font-size: 18px; color: #202124;">Next Steps</h2>
        <p>Complete your profile to get started.</p>
        <img src="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==" width="32" height="32" alt="icon" style="display: block; margin: 10px 0;">
    </div>
    <div class="footer">
        <p>This email was sent by Example Service.</p>
        <p>123 Main St, Anytown, USA</p>
    </div>
</div>
</body>
</html>"##;

            let prepared = crate::email::prepare_email_html(html, None, None);

            assert!(
                !prepared.html.contains("<script"),
                "script tags must be stripped"
            );
            assert!(
                !prepared.html.contains("onerror"),
                "event handlers must be stripped"
            );

            // The style block should be preserved
            assert!(
                prepared.html.contains("<style>"),
                "style block should be preserved"
            );

            let pixels = crate::pixbuf::render_to_rgba(&prepared.html, 600, 800);
            assert_eq!(pixels.len(), 600 * 800 * 4, "pixel buffer size mismatch");

            let has_opaque = pixels.chunks(4).any(|px| px[3] > 0);
            assert!(has_opaque, "render should produce non-transparent pixels");

            let has_non_white = pixels
                .chunks(4)
                .any(|px| px[3] > 0 && (px[0] < 250 || px[1] < 250 || px[2] < 250));
            assert!(
                has_non_white,
                "render should contain non-white pixels from actual content"
            );
        }

        #[cfg(feature = "email")]
        #[test]
        fn test_email_render_outlook_style() {
            let html = br##"<html>
<head>
    <meta charset="utf-8">
</head>
<body>
<!--[if mso]>
<style>table { border-collapse: collapse; }</style>
<![endif]-->
<script type="text/javascript">document.write('tracking')</script>
<table width="600" border="0" cellpadding="0" cellspacing="0" align="center" bgcolor="#ffffff">
<tr>
    <td align="center" valign="top" width="600" height="80" bgcolor="#1a3c6e">
        <font face="Times New Roman, serif" size="5" color="#ffffff"><b>Newsletter Title</b></font>
    </td>
</tr>
<tr>
    <td>
        <table width="100%" border="0" cellpadding="0" cellspacing="0">
        <tr>
            <td width="300" align="left" valign="top" style="padding: 10px;">
                <table width="100%" border="0" cellpadding="0" cellspacing="0">
                <tr>
                    <td bgcolor="#f5f5f5" style="padding: 15px;">
                        <font face="Arial" size="3" color="#333333"><b>Article Title</b></font><br>
                        <font face="Arial" size="2" color="#666666">
                            Lorem ipsum dolor sit amet, consectetur adipiscing elit.
                            Sed do eiusmod tempor incididunt ut labore.
                        </font>
                    </td>
                </tr>
                </table>
            </td>
            <td width="300" align="right" valign="top" style="padding: 10px;">
                <img src="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==" width="280" height="180" alt="Article Image" onclick="track()" onmouseover="highlight()">
            </td>
        </tr>
        </table>
    </td>
</tr>
<tr>
    <td align="center" valign="middle" height="50" bgcolor="#eeeeee">
        <font face="Verdana" size="1" color="#999999">
            &copy; 2024 Newsletter Corp. All rights reserved.
        </font>
    </td>
</tr>
</table>
</body>
</html>"##;

            let prepared = crate::email::prepare_email_html(html, None, None);

            // Script tag and its contents must be stripped
            assert!(
                !prepared.html.contains("<script"),
                "script tags must be stripped"
            );
            assert!(
                !prepared.html.contains("document.write"),
                "script contents must be stripped"
            );

            // Event handler attributes must be stripped
            assert!(
                !prepared.html.contains("onclick"),
                "onclick must be stripped"
            );
            assert!(
                !prepared.html.contains("onmouseover"),
                "onmouseover must be stripped"
            );

            // MSO conditional comments should pass through (they're just HTML comments)
            assert!(
                prepared.html.contains("<!--[if mso]>"),
                "MSO comments should be preserved"
            );

            // Normal content preserved
            assert!(
                prepared.html.contains("Newsletter Title"),
                "heading text must be preserved"
            );
            assert!(
                prepared.html.contains("Article Title"),
                "article text must be preserved"
            );

            let pixels = crate::pixbuf::render_to_rgba(&prepared.html, 600, 800);
            assert_eq!(pixels.len(), 600 * 800 * 4, "pixel buffer size mismatch");

            let has_opaque = pixels.chunks(4).any(|px| px[3] > 0);
            assert!(has_opaque, "render should produce non-transparent pixels");

            let has_non_white = pixels
                .chunks(4)
                .any(|px| px[3] > 0 && (px[0] < 250 || px[1] < 250 || px[2] < 250));
            assert!(
                has_non_white,
                "render should contain non-white pixels from actual content"
            );
        }
    }
}
