//! Text selection support for litehtml documents.
//!
//! Provides character-level hit testing and multi-element text selection.
//! Each word in litehtml is a separate text element with its own render-engine
//! placement, so hit testing and rectangle computation use the element's
//! placement directly rather than simulating line wrapping.
//!
//! # Safety
//!
//! `Selection` stores raw element pointers internally. The caller must ensure
//! the parent `Document` outlives the `Selection` — element pointers become
//! invalid once the document is dropped.
//!
//! # Usage
//!
//! The selection API accepts a text measurement function `&dyn Fn(&str, usize) -> f32`
//! rather than `&dyn DocumentContainer`, because `Document` already holds a mutable
//! borrow of the container. Consumers should capture their own measurement function
//! before creating the document, or use a separate measurement path.
//!
//! ```ignore
//! let measure = container.text_measure_fn();
//! let mut doc = Document::from_html(&html, &mut container, None, None)?;
//! doc.render(width);
//! selection.start_at(&doc, &measure, x, y, cx, cy);
//! ```

use crate::{Document, Element, Position};
use std::marker::PhantomData;

/// Text measurement function signature: `(text, font_handle) -> width_in_pixels`.
pub type MeasureTextFn<'a> = dyn Fn(&str, usize) -> f32 + 'a;

/// A position within a text element: which element and which character offset.
#[derive(Debug, Clone)]
pub struct SelectionEndpoint {
    element: *mut crate::sys::lh_element_t,
    pub char_index: usize,
    pub x: f32,
}

impl SelectionEndpoint {
    /// Get the element this endpoint refers to.
    ///
    /// # Safety (conceptual)
    ///
    /// The returned `Element` is only valid while the parent `Document` is alive.
    fn element(&self) -> Element<'_> {
        Element {
            ptr: self.element,
            _phantom: PhantomData,
        }
    }
}

/// Cached result of a document-order comparison between two elements.
#[derive(Clone)]
struct OrderCache {
    a: *mut crate::sys::lh_element_t,
    b: *mut crate::sys::lh_element_t,
    a_before_b: bool,
}

/// Text selection state for a litehtml document.
///
/// The `'doc` lifetime ties this selection to its parent [`Document`], preventing
/// use-after-free if the document is dropped while the selection holds element
/// pointers. Use [`Selection::for_document`] to create a lifetime-bound selection.
pub struct Selection<'doc> {
    start: Option<SelectionEndpoint>,
    end: Option<SelectionEndpoint>,
    rectangles: Vec<Position>,
    order_cache: Option<OrderCache>,
    _doc: PhantomData<&'doc ()>,
}

impl<'doc> Selection<'doc> {
    /// Create an empty (inactive) selection without lifetime enforcement.
    ///
    /// Prefer [`Selection::for_document`] to tie the selection lifetime to
    /// a specific document, preventing use-after-free at compile time.
    pub fn new() -> Self {
        Self {
            start: None,
            end: None,
            rectangles: Vec::new(),
            order_cache: None,
            _doc: PhantomData,
        }
    }

    /// Create an empty selection tied to a document's lifetime.
    ///
    /// The returned `Selection` cannot outlive `doc`, enforced by the compiler.
    /// The document is NOT borrowed persistently — only the lifetime is captured.
    pub fn for_document(_doc: &'doc Document<'_>) -> Self {
        Self::new()
    }

    /// Begin a selection at document coordinates `(x, y)`.
    ///
    /// `measure_text` should return the pixel width of a string rendered with
    /// the given font handle — typically wrapping `DocumentContainer::text_width`.
    pub fn start_at(
        &mut self,
        doc: &Document<'_>,
        measure_text: &MeasureTextFn<'_>,
        x: f32,
        y: f32,
        client_x: f32,
        client_y: f32,
    ) {
        self.clear();
        if let Some(endpoint) = hit_test_char(doc, measure_text, x, y, client_x, client_y) {
            self.start = Some(endpoint);
        }
    }

    /// Extend the selection to document coordinates `(x, y)`.
    ///
    /// Recomputes the selected text and highlight rectangles.
    pub fn extend_to(
        &mut self,
        doc: &Document<'_>,
        measure_text: &MeasureTextFn<'_>,
        x: f32,
        y: f32,
        client_x: f32,
        client_y: f32,
    ) {
        if self.start.is_none() {
            return;
        }
        if let Some(endpoint) = hit_test_char(doc, measure_text, x, y, client_x, client_y) {
            self.end = Some(endpoint);
            self.recompute_rectangles(measure_text);
        }
    }

    /// Clear the selection.
    pub fn clear(&mut self) {
        self.start = None;
        self.end = None;
        self.rectangles.clear();
        self.order_cache = None;
    }

    /// Returns `true` if there is an active selection with both start and end.
    pub fn is_active(&self) -> bool {
        self.start.is_some() && self.end.is_some()
    }

    /// Extract the selected text, walking the DOM between start and end.
    ///
    /// Returns `None` if the selection is not active.
    pub fn selected_text(&self) -> Option<String> {
        let start = self.start.as_ref()?;
        let end = self.end.as_ref()?;

        // Normalize into document order (use cache if available)
        let (first, second) = normalize_endpoints(start, end, &self.order_cache);
        let first_el = first.element();
        let second_el = second.element();

        // Same element: slice the text
        if first.element == second.element {
            let text = first_el.get_text();
            let (lo, hi) = ordered_indices(first.char_index, second.char_index);
            return Some(safe_char_slice(&text, lo, hi));
        }

        // Multi-element: walk from first to second, collecting text
        let mut result = String::new();

        // Text from first element (from char_index to end)
        let first_text = first_el.get_text();
        result.push_str(&safe_char_slice_from(&first_text, first.char_index));

        // Walk intermediate text nodes
        let mut current = next_text_leaf(&first_el, &second_el);
        while let Some(ref el) = current {
            if el.as_ptr() == second.element {
                break;
            }
            result.push_str(&el.get_text());
            current = next_text_leaf(el, &second_el);
        }

        // Text from second element (from 0 to char_index)
        let second_text = second_el.get_text();
        result.push_str(&safe_char_slice_to(&second_text, second.char_index));

        Some(result)
    }

    /// Highlight rectangles for the current selection.
    pub fn rectangles(&self) -> &[Position] {
        &self.rectangles
    }

    /// Recompute highlight rectangles based on current start/end.
    fn recompute_rectangles(&mut self, measure_text: &MeasureTextFn<'_>) {
        self.rectangles.clear();

        let (start, end) = match (self.start.as_ref(), self.end.as_ref()) {
            (Some(s), Some(e)) => (s, e),
            _ => return,
        };

        // Update order cache if endpoints changed
        if start.element != end.element {
            let needs_update = self
                .order_cache
                .as_ref()
                .is_none_or(|c| c.a != start.element || c.b != end.element);
            if needs_update {
                let a_before_b = is_before(&start.element(), &end.element());
                self.order_cache = Some(OrderCache {
                    a: start.element,
                    b: end.element,
                    a_before_b,
                });
            }
        }

        // Normalize into document order
        let (first, second) = normalize_endpoints(start, end, &self.order_cache);

        if first.element == second.element {
            let el = first.element();
            compute_text_rect(
                &el,
                measure_text,
                first.char_index,
                second.char_index,
                &mut self.rectangles,
            );
            return;
        }

        // First element: from char_index to end of text
        let first_el = first.element();
        let first_text = first_el.get_text();
        let first_len = first_text.chars().count();
        compute_text_rect(
            &first_el,
            measure_text,
            first.char_index,
            first_len,
            &mut self.rectangles,
        );

        // Intermediate elements: full highlight
        let second_el = second.element();
        let mut current = next_text_leaf(&first_el, &second_el);
        while let Some(ref el) = current {
            if el.as_ptr() == second.element {
                break;
            }
            let text = el.get_text();
            let len = text.chars().count();
            compute_text_rect(el, measure_text, 0, len, &mut self.rectangles);
            current = next_text_leaf(el, &second_el);
        }

        // Second element: from 0 to char_index
        compute_text_rect(
            &second_el,
            measure_text,
            0,
            second.char_index,
            &mut self.rectangles,
        );
    }
}

impl Default for Selection<'_> {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Document order
// ---------------------------------------------------------------------------

/// Determine if element `a` comes before `b` in document order.
/// Walks forward from `a` using `next_text_leaf`; if we reach `b`, then a < b.
fn is_before(a: &Element<'_>, b: &Element<'_>) -> bool {
    if a.as_ptr() == b.as_ptr() {
        return false;
    }
    let mut cursor = next_text_leaf(a, b);
    while let Some(ref el) = cursor {
        if el.as_ptr() == b.as_ptr() {
            return true;
        }
        cursor = next_text_leaf(el, b);
    }
    false
}

/// Normalize user-order endpoints into document order: returns (first, second).
///
/// Uses the cached order result when available to avoid repeated DOM walks.
fn normalize_endpoints<'a>(
    a: &'a SelectionEndpoint,
    b: &'a SelectionEndpoint,
    cache: &Option<OrderCache>,
) -> (&'a SelectionEndpoint, &'a SelectionEndpoint) {
    if a.element == b.element {
        if a.char_index <= b.char_index {
            (a, b)
        } else {
            (b, a)
        }
    } else {
        let a_before_b = cache
            .as_ref()
            .filter(|c| c.a == a.element && c.b == b.element)
            .map_or_else(|| is_before(&a.element(), &b.element()), |c| c.a_before_b);
        if a_before_b {
            (a, b)
        } else {
            (b, a)
        }
    }
}

// ---------------------------------------------------------------------------
// Hit testing
// ---------------------------------------------------------------------------

/// Get the font handle for a text element, falling back to its parent.
fn font_for_text(text_el: &Element<'_>) -> usize {
    let f = text_el.font();
    if f != 0 {
        return f;
    }
    text_el.parent().map_or(0, |p| p.font())
}

/// Get the placement for a text element. Falls back to the parent's placement
/// if the text element's own placement has zero width (some text nodes don't
/// have their own render-item position).
fn placement_for_text(text_el: &Element<'_>) -> Position {
    let p = text_el.placement();
    if p.width > 0.0 {
        return p;
    }
    text_el.parent().map_or(p, |parent| parent.placement())
}

/// Character-level hit testing: find which character in which element is at (x, y).
///
/// Uses the text element's own render-engine placement — litehtml splits text
/// into per-word elements and positions each one during layout.
fn hit_test_char(
    doc: &Document<'_>,
    measure_text: &MeasureTextFn<'_>,
    x: f32,
    y: f32,
    client_x: f32,
    client_y: f32,
) -> Option<SelectionEndpoint> {
    let el = doc.get_element_by_point(x, y, client_x, client_y)?;

    let text_el = if el.is_text() {
        el
    } else {
        closest_text_leaf(&el, x, y).or_else(|| first_text_leaf(&el))?
    };

    let text = text_el.get_text();
    if text.is_empty() || text.trim().is_empty() {
        return Some(SelectionEndpoint {
            element: text_el.as_ptr(),
            char_index: 0,
            x,
        });
    }

    let font = font_for_text(&text_el);
    let placement = placement_for_text(&text_el);

    let local_x = x - placement.x;
    let char_index = find_char_at_x(measure_text, &text, font, local_x);

    Some(SelectionEndpoint {
        element: text_el.as_ptr(),
        char_index,
        x,
    })
}

/// Find which character index corresponds to pixel offset `target_x` within
/// the given text rendered with `font`.
///
/// Builds the prefix string incrementally to avoid O(n) allocations per call.
fn find_char_at_x(
    measure_text: &MeasureTextFn<'_>,
    text: &str,
    font: usize,
    target_x: f32,
) -> usize {
    if text.is_empty() || target_x <= 0.0 {
        return 0;
    }

    let mut prefix = String::with_capacity(text.len());
    let mut prev_width = 0.0f32;
    let mut count = 0;

    for ch in text.chars() {
        prefix.push(ch);
        count += 1;
        let width = measure_text(&prefix, font);
        let midpoint = (prev_width + width) / 2.0;

        if target_x < midpoint {
            return count - 1;
        }
        prev_width = width;
    }

    count
}

// ---------------------------------------------------------------------------
// Tree walking
// ---------------------------------------------------------------------------

/// Descend to the first text leaf child of `el`.
fn first_text_leaf<'a>(el: &Element<'a>) -> Option<Element<'a>> {
    if el.is_text() {
        return Some(Element {
            ptr: el.as_ptr(),
            _phantom: PhantomData,
        });
    }
    let count = el.children_count();
    for i in 0..count {
        if let Some(child) = el.child_at(i) {
            if let Some(leaf) = first_text_leaf(&child) {
                return Some(leaf);
            }
        }
    }
    None
}

/// Returns true if this text element contains only whitespace.
fn is_whitespace_only(el: &Element<'_>) -> bool {
    el.get_text().trim().is_empty()
}

/// Find the closest non-whitespace text leaf descendant by both Y and X.
///
/// First filters to text leaves whose placement vertically overlaps or is
/// closest to `target_y`, then among those picks the one closest to `target_x`.
fn closest_text_leaf<'a>(el: &Element<'a>, target_x: f32, target_y: f32) -> Option<Element<'a>> {
    let mut candidates: Vec<(Element<'a>, Position)> = Vec::new();

    fn collect<'a>(el: &Element<'a>, out: &mut Vec<(Element<'a>, Position)>) {
        if el.is_text() && !is_whitespace_only(el) {
            let p = placement_for_text(el);
            out.push((
                Element {
                    ptr: el.as_ptr(),
                    _phantom: PhantomData,
                },
                p,
            ));
            return;
        }
        let count = el.children_count();
        for i in 0..count {
            if let Some(child) = el.child_at(i) {
                collect(&child, out);
            }
        }
    }

    collect(el, &mut candidates);
    if candidates.is_empty() {
        return None;
    }

    // Find minimum Y distance
    let min_y_dist = candidates
        .iter()
        .map(|(_, p)| {
            if target_y >= p.y && target_y < p.y + p.height {
                0.0f32
            } else {
                let center = p.y + p.height / 2.0;
                (target_y - center).abs()
            }
        })
        .fold(f32::MAX, f32::min);

    // Filter to candidates within a small tolerance of the best Y distance
    let y_tolerance = 2.0;
    let y_candidates: Vec<_> = candidates
        .into_iter()
        .filter(|(_, p)| {
            let d = if target_y >= p.y && target_y < p.y + p.height {
                0.0f32
            } else {
                let center = p.y + p.height / 2.0;
                (target_y - center).abs()
            };
            d <= min_y_dist + y_tolerance
        })
        .collect();

    // Among Y-matching candidates, pick closest by X
    y_candidates
        .into_iter()
        .min_by(|(_, a), (_, b)| {
            let da = if target_x >= a.x && target_x < a.x + a.width {
                0.0f32
            } else {
                let center = a.x + a.width / 2.0;
                (target_x - center).abs()
            };
            let db = if target_x >= b.x && target_x < b.x + b.width {
                0.0f32
            } else {
                let center = b.x + b.width / 2.0;
                (target_x - center).abs()
            };
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(el, _)| el)
}

/// Maximum ancestor levels to traverse before giving up.
/// Prevents infinite loops on malformed DOMs.
const MAX_TREE_DEPTH: usize = 256;

/// Walk to the next text leaf after `el`, stopping before `stop`.
/// Walks up to the parent, then to the next sibling, then descends.
/// Gives up after [`MAX_TREE_DEPTH`] ancestor levels to guard against
/// malformed or extremely deep DOMs.
fn next_text_leaf<'a>(el: &Element<'a>, stop: &Element<'a>) -> Option<Element<'a>> {
    let mut current_ptr = el.as_ptr();

    for _ in 0..MAX_TREE_DEPTH {
        let current = Element {
            ptr: current_ptr,
            _phantom: PhantomData,
        };
        let parent = current.parent()?;
        let sibling_count = parent.children_count();

        // Find current's index among parent's children
        let mut found_idx = None;
        for i in 0..sibling_count {
            if let Some(child) = parent.child_at(i) {
                if child.as_ptr() == current_ptr {
                    found_idx = Some(i);
                    break;
                }
            }
        }

        if let Some(idx) = found_idx {
            for i in (idx + 1)..sibling_count {
                if let Some(sibling) = parent.child_at(i) {
                    if sibling.as_ptr() == stop.as_ptr() {
                        return Some(sibling);
                    }
                    if let Some(leaf) = first_text_leaf(&sibling) {
                        if leaf.as_ptr() == stop.as_ptr() {
                            return Some(leaf);
                        }
                        return Some(leaf);
                    }
                }
            }
        }

        // No more siblings at this level, walk up
        current_ptr = parent.as_ptr();
    }

    None
}

// ---------------------------------------------------------------------------
// Rectangle computation
// ---------------------------------------------------------------------------

/// Compute a highlight rectangle for a character range within a single text element.
///
/// Uses the element's render-engine placement directly.
fn compute_text_rect(
    el: &Element<'_>,
    measure_text: &MeasureTextFn<'_>,
    from_char: usize,
    to_char: usize,
    out: &mut Vec<Position>,
) {
    let (lo, hi) = ordered_indices(from_char, to_char);
    if lo == hi {
        return;
    }

    let text = el.get_text();
    if text.trim().is_empty() {
        return;
    }
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return;
    }

    let font = font_for_text(el);
    let placement = placement_for_text(el);

    let lo = lo.min(chars.len());
    let hi = hi.min(chars.len());

    let start_px = if lo == 0 {
        0.0
    } else {
        let prefix: String = chars[..lo].iter().collect();
        measure_text(&prefix, font)
    };

    let end_px = {
        let prefix: String = chars[..hi].iter().collect();
        measure_text(&prefix, font)
    };

    if end_px > start_px {
        out.push(Position {
            x: placement.x + start_px,
            y: placement.y,
            width: end_px - start_px,
            height: placement.height,
        });
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn ordered_indices(a: usize, b: usize) -> (usize, usize) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}

fn safe_char_slice(text: &str, from: usize, to: usize) -> String {
    text.chars()
        .skip(from)
        .take(to.saturating_sub(from))
        .collect()
}

fn safe_char_slice_from(text: &str, from: usize) -> String {
    text.chars().skip(from).collect()
}

fn safe_char_slice_to(text: &str, to: usize) -> String {
    text.chars().take(to).collect()
}
