/*
 * litehtml C API wrapper
 *
 * Provides a C-compatible interface to the litehtml C++ HTML/CSS rendering
 * engine, suitable for FFI from Rust or other languages.
 */

#ifndef LITEHTML_C_H
#define LITEHTML_C_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/* --------------------------------------------------------------------------
 * Simple C structs (value types, passed across FFI by value or pointer)
 * -------------------------------------------------------------------------- */

typedef struct lh_position {
    float x;
    float y;
    float width;
    float height;
} lh_position_t;

typedef struct lh_size {
    float width;
    float height;
} lh_size_t;

typedef struct lh_web_color {
    unsigned char red;
    unsigned char green;
    unsigned char blue;
    unsigned char alpha;
    int is_current_color;
} lh_web_color_t;

typedef struct lh_font_metrics {
    float font_size;
    float height;
    float ascent;
    float descent;
    float x_height;
    float ch_width;
    int   draw_spaces;
    float sub_shift;
    float super_shift;
} lh_font_metrics_t;

typedef struct lh_border_radiuses {
    float top_left_x;
    float top_left_y;
    float top_right_x;
    float top_right_y;
    float bottom_right_x;
    float bottom_right_y;
    float bottom_left_x;
    float bottom_left_y;
} lh_border_radiuses_t;

typedef struct lh_border {
    float          width;
    int            style;
    lh_web_color_t color;
} lh_border_t;

typedef struct lh_borders {
    lh_border_t         left;
    lh_border_t         top;
    lh_border_t         right;
    lh_border_t         bottom;
    lh_border_radiuses_t radius;
} lh_borders_t;

typedef struct lh_media_features {
    int   type;
    float width;
    float height;
    float device_width;
    float device_height;
    int   color;
    int   color_index;
    int   monochrome;
    float resolution;
} lh_media_features_t;

typedef struct lh_point {
    float x;
    float y;
} lh_point_t;

/* --------------------------------------------------------------------------
 * Opaque types (pointers to C++ objects, never dereferenced from C)
 * -------------------------------------------------------------------------- */

typedef struct lh_document           lh_document_t;
typedef struct lh_background_layer   lh_background_layer_t;
typedef struct lh_linear_gradient    lh_linear_gradient_t;
typedef struct lh_radial_gradient    lh_radial_gradient_t;
typedef struct lh_conic_gradient     lh_conic_gradient_t;
typedef struct lh_font_description   lh_font_description_t;
typedef struct lh_list_marker        lh_list_marker_t;

/* --------------------------------------------------------------------------
 * Accessor functions for opaque types
 * -------------------------------------------------------------------------- */

/* font_description getters */
const char* lh_font_description_family(const lh_font_description_t* fd);
float       lh_font_description_size(const lh_font_description_t* fd);
int         lh_font_description_style(const lh_font_description_t* fd);
int         lh_font_description_weight(const lh_font_description_t* fd);
int         lh_font_description_decoration_line(const lh_font_description_t* fd);
int   lh_font_description_decoration_thickness_is_predefined(const lh_font_description_t* fd);
int   lh_font_description_decoration_thickness_predef(const lh_font_description_t* fd);
float lh_font_description_decoration_thickness_value(const lh_font_description_t* fd);
int   lh_font_description_decoration_style(const lh_font_description_t* fd);
lh_web_color_t lh_font_description_decoration_color(const lh_font_description_t* fd);
const char* lh_font_description_emphasis_style(const lh_font_description_t* fd);
lh_web_color_t lh_font_description_emphasis_color(const lh_font_description_t* fd);
int   lh_font_description_emphasis_position(const lh_font_description_t* fd);

/* list_marker getters */
const char*    lh_list_marker_image(const lh_list_marker_t* m);
const char*    lh_list_marker_baseurl(const lh_list_marker_t* m);
int            lh_list_marker_type(const lh_list_marker_t* m);
lh_web_color_t lh_list_marker_color(const lh_list_marker_t* m);
lh_position_t  lh_list_marker_pos(const lh_list_marker_t* m);
int            lh_list_marker_index(const lh_list_marker_t* m);
uintptr_t      lh_list_marker_font(const lh_list_marker_t* m);

/* background_layer getters */
lh_position_t        lh_background_layer_border_box(const lh_background_layer_t* layer);
lh_border_radiuses_t lh_background_layer_border_radius(const lh_background_layer_t* layer);
lh_position_t        lh_background_layer_clip_box(const lh_background_layer_t* layer);
lh_position_t        lh_background_layer_origin_box(const lh_background_layer_t* layer);
int                  lh_background_layer_attachment(const lh_background_layer_t* layer);
int                  lh_background_layer_repeat(const lh_background_layer_t* layer);
int                  lh_background_layer_is_root(const lh_background_layer_t* layer);

/* linear_gradient getters */
lh_point_t     lh_linear_gradient_start(const lh_linear_gradient_t* g);
lh_point_t     lh_linear_gradient_end(const lh_linear_gradient_t* g);
int            lh_linear_gradient_color_points_count(const lh_linear_gradient_t* g);
float          lh_linear_gradient_color_point_offset(const lh_linear_gradient_t* g, int idx);
lh_web_color_t lh_linear_gradient_color_point_color(const lh_linear_gradient_t* g, int idx);
int            lh_linear_gradient_color_space(const lh_linear_gradient_t* g);
int            lh_linear_gradient_hue_interpolation(const lh_linear_gradient_t* g);

/* radial_gradient getters */
lh_point_t     lh_radial_gradient_position(const lh_radial_gradient_t* g);
lh_point_t     lh_radial_gradient_radius(const lh_radial_gradient_t* g);
int            lh_radial_gradient_color_points_count(const lh_radial_gradient_t* g);
float          lh_radial_gradient_color_point_offset(const lh_radial_gradient_t* g, int idx);
lh_web_color_t lh_radial_gradient_color_point_color(const lh_radial_gradient_t* g, int idx);
int            lh_radial_gradient_color_space(const lh_radial_gradient_t* g);
int            lh_radial_gradient_hue_interpolation(const lh_radial_gradient_t* g);

/* conic_gradient getters */
lh_point_t     lh_conic_gradient_position(const lh_conic_gradient_t* g);
float          lh_conic_gradient_angle(const lh_conic_gradient_t* g);
float          lh_conic_gradient_radius(const lh_conic_gradient_t* g);
int            lh_conic_gradient_color_points_count(const lh_conic_gradient_t* g);
float          lh_conic_gradient_color_point_offset(const lh_conic_gradient_t* g, int idx);
lh_web_color_t lh_conic_gradient_color_point_color(const lh_conic_gradient_t* g, int idx);
int            lh_conic_gradient_color_space(const lh_conic_gradient_t* g);
int            lh_conic_gradient_hue_interpolation(const lh_conic_gradient_t* g);

/* --------------------------------------------------------------------------
 * Container callback vtable
 *
 * Each function pointer receives void* user_data as its first argument.
 * Opaque C++ objects are passed as opaque pointers; use the accessor
 * functions above to extract data from them.
 * -------------------------------------------------------------------------- */

typedef void (*lh_set_string_fn)(void* ctx, const char* text);
typedef void (*lh_set_language_fn)(void* ctx, const char* language, const char* culture);

typedef struct lh_container_vtable {
    uintptr_t   (*create_font)(void* user_data,
                               const lh_font_description_t* descr,
                               lh_font_metrics_t* fm);

    void        (*delete_font)(void* user_data,
                               uintptr_t hFont);

    float       (*text_width)(void* user_data,
                              const char* text,
                              uintptr_t hFont);

    void        (*draw_text)(void* user_data,
                             uintptr_t hdc,
                             const char* text,
                             uintptr_t hFont,
                             lh_web_color_t color,
                             lh_position_t pos);

    float       (*pt_to_px)(void* user_data,
                            float pt);

    float       (*get_default_font_size)(void* user_data);

    const char* (*get_default_font_name)(void* user_data);

    void        (*draw_list_marker)(void* user_data,
                                    uintptr_t hdc,
                                    const lh_list_marker_t* marker);

    void        (*load_image)(void* user_data,
                              const char* src,
                              const char* baseurl,
                              int redraw_on_ready);

    void        (*get_image_size)(void* user_data,
                                  const char* src,
                                  const char* baseurl,
                                  lh_size_t* sz);

    void        (*draw_image)(void* user_data,
                              uintptr_t hdc,
                              const lh_background_layer_t* layer,
                              const char* url,
                              const char* base_url);

    void        (*draw_solid_fill)(void* user_data,
                                   uintptr_t hdc,
                                   const lh_background_layer_t* layer,
                                   lh_web_color_t color);

    void        (*draw_linear_gradient)(void* user_data,
                                        uintptr_t hdc,
                                        const lh_background_layer_t* layer,
                                        const lh_linear_gradient_t* gradient);

    void        (*draw_radial_gradient)(void* user_data,
                                        uintptr_t hdc,
                                        const lh_background_layer_t* layer,
                                        const lh_radial_gradient_t* gradient);

    void        (*draw_conic_gradient)(void* user_data,
                                       uintptr_t hdc,
                                       const lh_background_layer_t* layer,
                                       const lh_conic_gradient_t* gradient);

    void        (*draw_borders)(void* user_data,
                                uintptr_t hdc,
                                lh_borders_t borders,
                                lh_position_t draw_pos,
                                int root);

    void        (*set_caption)(void* user_data,
                               const char* caption);

    void        (*set_base_url)(void* user_data,
                                const char* base_url);

    void        (*link)(void* user_data);

    void        (*on_anchor_click)(void* user_data,
                                   const char* url);

    void        (*on_mouse_event)(void* user_data,
                                  int event);

    void        (*set_cursor)(void* user_data,
                              const char* cursor);

    void        (*transform_text)(void* user_data,
                                  const char* text,
                                  int tt,
                                  lh_set_string_fn set_result,
                                  void* ctx);

    void        (*import_css)(void* user_data,
                              const char* url,
                              const char* baseurl,
                              lh_set_string_fn set_result,
                              void* ctx);

    void        (*set_clip)(void* user_data,
                            lh_position_t pos,
                            lh_border_radiuses_t bdr_radius);

    void        (*del_clip)(void* user_data);

    void        (*get_viewport)(void* user_data,
                                lh_position_t* viewport);

    void        (*get_media_features)(void* user_data,
                                      lh_media_features_t* media);

    void        (*get_language)(void* user_data,
                                lh_set_language_fn set_result,
                                void* ctx);
} lh_container_vtable_t;

/* Element handle -- borrowed pointer, valid while the parent document is alive */
typedef struct lh_element lh_element_t;

/* --------------------------------------------------------------------------
 * Document lifecycle
 * -------------------------------------------------------------------------- */

lh_document_t* lh_document_create_from_string(
    const char* html,
    lh_container_vtable_t* vtable,
    void* user_data,
    const char* master_css,
    const char* user_styles);

void  lh_document_destroy(lh_document_t* doc);
float lh_document_render(lh_document_t* doc, float max_width);

void  lh_document_draw(lh_document_t* doc,
                        uintptr_t hdc,
                        float x,
                        float y,
                        const lh_position_t* clip);

float lh_document_width(const lh_document_t* doc);
float lh_document_height(const lh_document_t* doc);

/* Add and immediately apply a CSS stylesheet to the document.
   Requires a subsequent render() to update layout. */
void lh_document_add_stylesheet(lh_document_t* doc,
                                const char* css_text,
                                const char* baseurl,
                                const char* media);

/* Get the root element of the document. Returns NULL if doc is NULL. */
lh_element_t* lh_document_root(lh_document_t* doc);

/* --------------------------------------------------------------------------
 * Element introspection
 * -------------------------------------------------------------------------- */

/* Get the parent element. Returns NULL for root or if el is NULL. */
lh_element_t* lh_element_parent(lh_element_t* el);

/* Number of child elements. Returns 0 if el is NULL. */
int lh_element_children_count(lh_element_t* el);

/* Get the child at the given index. Returns NULL if out of bounds. */
lh_element_t* lh_element_child_at(lh_element_t* el, int index);

/* Returns non-zero if the element is a text node. */
int lh_element_is_text(lh_element_t* el);

/* Get the font handle from the element's computed CSS. Returns 0 on error. */
uintptr_t lh_element_get_font(lh_element_t* el);

/* Get the font size from the element's computed CSS. Returns 0.0 on error. */
float lh_element_get_font_size(lh_element_t* el);

/* Get the element's absolute pixel bounding box after layout. */
void lh_element_get_placement(lh_element_t* el, lh_position_t* pos);

/* Get the element's recursive text content via callback. */
void lh_element_get_text(lh_element_t* el,
                         void (*cb)(void* ctx, const char* text),
                         void* ctx);

/* Hit testing: find the deepest element at document coordinates (x, y). */
lh_element_t* lh_document_get_element_by_point(lh_document_t* doc,
                                                float x, float y,
                                                float client_x, float client_y);

/* Number of per-line inline boxes for a rendered element (0 if not inline). */
int lh_element_get_inline_boxes_count(lh_element_t* el);

/* Get the i-th inline box in absolute document coordinates. */
void lh_element_get_inline_box_at(lh_element_t* el, int index, lh_position_t* pos);

/* Get all inline boxes in one call via callback. Avoids recomputing boxes N+1 times.
   The callback receives each box in absolute document coordinates plus a user context. */
typedef void (*lh_inline_box_callback)(const lh_position_t* pos, void* ctx);
void lh_element_get_inline_boxes(lh_element_t* el, lh_inline_box_callback cb, void* ctx);

/* Get the computed text-align value (0=left, 1=right, 2=center, 3=justify). */
int lh_element_get_text_align(lh_element_t* el);

/* Get the computed line-height in pixels. */
float lh_element_get_line_height(lh_element_t* el);

/* Parse an HTML fragment and append the resulting elements as children of parent.
   If replace_existing is non-zero, existing children are removed first.
   Requires a subsequent render() to update layout. */
void lh_document_append_children_from_string(lh_document_t* doc,
                                              lh_element_t* parent,
                                              const char* html,
                                              int replace_existing);

/* --------------------------------------------------------------------------
 * Mouse / interaction
 * -------------------------------------------------------------------------- */

int lh_document_on_mouse_over(lh_document_t* doc,
                               float x, float y,
                               float client_x, float client_y);

int lh_document_on_lbutton_down(lh_document_t* doc,
                                 float x, float y,
                                 float client_x, float client_y);

int lh_document_on_lbutton_up(lh_document_t* doc,
                               float x, float y,
                               float client_x, float client_y);

int lh_document_on_mouse_leave(lh_document_t* doc);
int lh_document_media_changed(lh_document_t* doc);

#ifdef __cplusplus
}
#endif

#endif /* LITEHTML_C_H */
