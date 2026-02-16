/*
 * litehtml C API wrapper -- implementation
 *
 * Bridges the C vtable-based interface to the C++ document_container
 * virtual class required by litehtml.
 */

#include "litehtml_c.h"
#include <litehtml.h>
#include <litehtml/render_item.h>
#include <cstring>
#include <string>

/* --------------------------------------------------------------------------
 * Conversion helpers between C structs and litehtml C++ types
 * -------------------------------------------------------------------------- */

static lh_position_t to_c(const litehtml::position& p)
{
    lh_position_t r;
    r.x      = p.x;
    r.y      = p.y;
    r.width  = p.width;
    r.height = p.height;
    return r;
}

static litehtml::position to_cpp(const lh_position_t& p)
{
    return litehtml::position(p.x, p.y, p.width, p.height);
}

static lh_size_t to_c(const litehtml::size& s)
{
    lh_size_t r;
    r.width  = s.width;
    r.height = s.height;
    return r;
}

static litehtml::size to_cpp(const lh_size_t& s)
{
    return litehtml::size(s.width, s.height);
}

static lh_web_color_t to_c(const litehtml::web_color& c)
{
    lh_web_color_t r;
    r.red              = c.red;
    r.green            = c.green;
    r.blue             = c.blue;
    r.alpha            = c.alpha;
    r.is_current_color = c.is_current_color ? 1 : 0;
    return r;
}

static litehtml::web_color to_cpp(const lh_web_color_t& c)
{
    litehtml::web_color r(c.red, c.green, c.blue, c.alpha);
    r.is_current_color = (c.is_current_color != 0);
    return r;
}

static lh_font_metrics_t to_c(const litehtml::font_metrics& m)
{
    lh_font_metrics_t r;
    r.font_size   = m.font_size;
    r.height      = m.height;
    r.ascent      = m.ascent;
    r.descent     = m.descent;
    r.x_height    = m.x_height;
    r.ch_width    = m.ch_width;
    r.draw_spaces = m.draw_spaces ? 1 : 0;
    r.sub_shift   = m.sub_shift;
    r.super_shift = m.super_shift;
    return r;
}

static void from_c(const lh_font_metrics_t& c, litehtml::font_metrics& out)
{
    out.font_size   = c.font_size;
    out.height      = c.height;
    out.ascent      = c.ascent;
    out.descent     = c.descent;
    out.x_height    = c.x_height;
    out.ch_width    = c.ch_width;
    out.draw_spaces = (c.draw_spaces != 0);
    out.sub_shift   = c.sub_shift;
    out.super_shift = c.super_shift;
}

static lh_border_radiuses_t to_c(const litehtml::border_radiuses& br)
{
    lh_border_radiuses_t r;
    r.top_left_x     = br.top_left_x;
    r.top_left_y     = br.top_left_y;
    r.top_right_x    = br.top_right_x;
    r.top_right_y    = br.top_right_y;
    r.bottom_right_x = br.bottom_right_x;
    r.bottom_right_y = br.bottom_right_y;
    r.bottom_left_x  = br.bottom_left_x;
    r.bottom_left_y  = br.bottom_left_y;
    return r;
}

static litehtml::border_radiuses to_cpp(const lh_border_radiuses_t& br)
{
    litehtml::border_radiuses r;
    r.top_left_x     = br.top_left_x;
    r.top_left_y     = br.top_left_y;
    r.top_right_x    = br.top_right_x;
    r.top_right_y    = br.top_right_y;
    r.bottom_right_x = br.bottom_right_x;
    r.bottom_right_y = br.bottom_right_y;
    r.bottom_left_x  = br.bottom_left_x;
    r.bottom_left_y  = br.bottom_left_y;
    return r;
}

static lh_border_t to_c(const litehtml::border& b)
{
    lh_border_t r;
    r.width = b.width;
    r.style = static_cast<int>(b.style);
    r.color = to_c(b.color);
    return r;
}

static lh_borders_t to_c(const litehtml::borders& b)
{
    lh_borders_t r;
    r.left   = to_c(b.left);
    r.top    = to_c(b.top);
    r.right  = to_c(b.right);
    r.bottom = to_c(b.bottom);
    r.radius = to_c(b.radius);
    return r;
}

static lh_media_features_t to_c(const litehtml::media_features& mf)
{
    lh_media_features_t r;
    r.type          = static_cast<int>(mf.type);
    r.width         = mf.width;
    r.height        = mf.height;
    r.device_width  = mf.device_width;
    r.device_height = mf.device_height;
    r.color         = mf.color;
    r.color_index   = mf.color_index;
    r.monochrome    = mf.monochrome;
    r.resolution    = mf.resolution;
    return r;
}

static void from_c(const lh_media_features_t& c, litehtml::media_features& out)
{
    out.type          = static_cast<litehtml::media_type>(c.type);
    out.width         = c.width;
    out.height        = c.height;
    out.device_width  = c.device_width;
    out.device_height = c.device_height;
    out.color         = c.color;
    out.color_index   = c.color_index;
    out.monochrome    = c.monochrome;
    out.resolution    = c.resolution;
}

static lh_point_t to_c(const litehtml::pointF& p)
{
    lh_point_t r;
    r.x = p.x;
    r.y = p.y;
    return r;
}

/* --------------------------------------------------------------------------
 * Internal document wrapper
 * -------------------------------------------------------------------------- */

class CDocumentContainer;

struct lh_document_internal
{
    litehtml::document::ptr  doc;
    CDocumentContainer*      container;
};

/* --------------------------------------------------------------------------
 * CDocumentContainer -- bridges vtable calls to the C callback table
 * -------------------------------------------------------------------------- */

class CDocumentContainer : public litehtml::document_container
{
public:
    lh_container_vtable_t* vtable;
    void*                  user_data;

    CDocumentContainer(lh_container_vtable_t* vt, void* ud)
        : vtable(vt), user_data(ud) {}

    /* -- create_font -- */
    litehtml::uint_ptr create_font(
        const litehtml::font_description& descr,
        const litehtml::document* /*doc*/,
        litehtml::font_metrics* fm) override
    {
        if (!vtable->create_font) return 0;

        auto* fd = reinterpret_cast<const lh_font_description_t*>(&descr);
        lh_font_metrics_t c_fm = {};

        litehtml::uint_ptr result = vtable->create_font(user_data, fd, &c_fm);

        if (fm)
            from_c(c_fm, *fm);

        return result;
    }

    /* -- delete_font -- */
    void delete_font(litehtml::uint_ptr hFont) override
    {
        if (vtable->delete_font)
            vtable->delete_font(user_data, hFont);
    }

    /* -- text_width -- */
    litehtml::pixel_t text_width(const char* text, litehtml::uint_ptr hFont) override
    {
        if (!vtable->text_width) return 0;
        return vtable->text_width(user_data, text, hFont);
    }

    /* -- draw_text -- */
    void draw_text(litehtml::uint_ptr hdc,
                   const char* text,
                   litehtml::uint_ptr hFont,
                   litehtml::web_color color,
                   const litehtml::position& pos) override
    {
        if (!vtable->draw_text) return;
        vtable->draw_text(user_data, hdc, text, hFont, to_c(color), to_c(pos));
    }

    /* -- pt_to_px -- */
    litehtml::pixel_t pt_to_px(float pt) const override
    {
        if (!vtable->pt_to_px) return pt;
        return vtable->pt_to_px(user_data, pt);
    }

    /* -- get_default_font_size -- */
    litehtml::pixel_t get_default_font_size() const override
    {
        if (!vtable->get_default_font_size) return 16;
        return vtable->get_default_font_size(user_data);
    }

    /* -- get_default_font_name -- */
    const char* get_default_font_name() const override
    {
        if (!vtable->get_default_font_name) return "serif";
        return vtable->get_default_font_name(user_data);
    }

    /* -- draw_list_marker -- */
    void draw_list_marker(litehtml::uint_ptr hdc,
                          const litehtml::list_marker& marker) override
    {
        if (!vtable->draw_list_marker) return;
        auto* m = reinterpret_cast<const lh_list_marker_t*>(&marker);
        vtable->draw_list_marker(user_data, hdc, m);
    }

    /* -- load_image -- */
    void load_image(const char* src, const char* baseurl,
                    bool redraw_on_ready) override
    {
        if (!vtable->load_image) return;
        vtable->load_image(user_data, src, baseurl, redraw_on_ready ? 1 : 0);
    }

    /* -- get_image_size -- */
    void get_image_size(const char* src, const char* baseurl,
                        litehtml::size& sz) override
    {
        if (!vtable->get_image_size) return;
        lh_size_t c_sz = to_c(sz);
        vtable->get_image_size(user_data, src, baseurl, &c_sz);
        sz.width  = c_sz.width;
        sz.height = c_sz.height;
    }

    /* -- draw_image -- */
    void draw_image(litehtml::uint_ptr hdc,
                    const litehtml::background_layer& layer,
                    const std::string& url,
                    const std::string& base_url) override
    {
        if (!vtable->draw_image) return;
        auto* bl = reinterpret_cast<const lh_background_layer_t*>(&layer);
        vtable->draw_image(user_data, hdc, bl, url.c_str(), base_url.c_str());
    }

    /* -- draw_solid_fill -- */
    void draw_solid_fill(litehtml::uint_ptr hdc,
                         const litehtml::background_layer& layer,
                         const litehtml::web_color& color) override
    {
        if (!vtable->draw_solid_fill) return;
        auto* bl = reinterpret_cast<const lh_background_layer_t*>(&layer);
        vtable->draw_solid_fill(user_data, hdc, bl, to_c(color));
    }

    /* -- draw_linear_gradient -- */
    void draw_linear_gradient(
        litehtml::uint_ptr hdc,
        const litehtml::background_layer& layer,
        const litehtml::background_layer::linear_gradient& gradient) override
    {
        if (!vtable->draw_linear_gradient) return;
        auto* bl = reinterpret_cast<const lh_background_layer_t*>(&layer);
        auto* lg = reinterpret_cast<const lh_linear_gradient_t*>(&gradient);
        vtable->draw_linear_gradient(user_data, hdc, bl, lg);
    }

    /* -- draw_radial_gradient -- */
    void draw_radial_gradient(
        litehtml::uint_ptr hdc,
        const litehtml::background_layer& layer,
        const litehtml::background_layer::radial_gradient& gradient) override
    {
        if (!vtable->draw_radial_gradient) return;
        auto* bl = reinterpret_cast<const lh_background_layer_t*>(&layer);
        auto* rg = reinterpret_cast<const lh_radial_gradient_t*>(&gradient);
        vtable->draw_radial_gradient(user_data, hdc, bl, rg);
    }

    /* -- draw_conic_gradient -- */
    void draw_conic_gradient(
        litehtml::uint_ptr hdc,
        const litehtml::background_layer& layer,
        const litehtml::background_layer::conic_gradient& gradient) override
    {
        if (!vtable->draw_conic_gradient) return;
        auto* bl = reinterpret_cast<const lh_background_layer_t*>(&layer);
        auto* cg = reinterpret_cast<const lh_conic_gradient_t*>(&gradient);
        vtable->draw_conic_gradient(user_data, hdc, bl, cg);
    }

    /* -- draw_borders -- */
    void draw_borders(litehtml::uint_ptr hdc,
                      const litehtml::borders& borders,
                      const litehtml::position& draw_pos,
                      bool root) override
    {
        if (!vtable->draw_borders) return;
        vtable->draw_borders(user_data, hdc,
                             to_c(borders), to_c(draw_pos),
                             root ? 1 : 0);
    }

    /* -- set_caption -- */
    void set_caption(const char* caption) override
    {
        if (vtable->set_caption)
            vtable->set_caption(user_data, caption);
    }

    /* -- set_base_url -- */
    void set_base_url(const char* base_url) override
    {
        if (vtable->set_base_url)
            vtable->set_base_url(user_data, base_url);
    }

    /* -- link -- */
    void link(const std::shared_ptr<litehtml::document>& /*doc*/,
              const litehtml::element::ptr& /*el*/) override
    {
        if (vtable->link)
            vtable->link(user_data);
    }

    /* -- on_anchor_click -- */
    void on_anchor_click(const char* url,
                         const litehtml::element::ptr& /*el*/) override
    {
        if (vtable->on_anchor_click)
            vtable->on_anchor_click(user_data, url);
    }

    /* -- on_mouse_event -- */
    void on_mouse_event(const litehtml::element::ptr& /*el*/,
                        litehtml::mouse_event event) override
    {
        if (vtable->on_mouse_event)
            vtable->on_mouse_event(user_data, static_cast<int>(event));
    }

    /* -- set_cursor -- */
    void set_cursor(const char* cursor) override
    {
        if (vtable->set_cursor)
            vtable->set_cursor(user_data, cursor);
    }

    /* -- transform_text -- */
    void transform_text(litehtml::string& text,
                        litehtml::text_transform tt) override
    {
        if (!vtable->transform_text) return;

        litehtml::string result = text;
        litehtml::string* result_ptr = &result;

        vtable->transform_text(
            user_data,
            text.c_str(),
            static_cast<int>(tt),
            [](void* ctx, const char* r) {
                auto* p = static_cast<litehtml::string*>(ctx);
                *p = r ? r : "";
            },
            result_ptr);

        text = result;
    }

    /* -- import_css -- */
    void import_css(litehtml::string& text,
                    const litehtml::string& url,
                    litehtml::string& baseurl) override
    {
        if (!vtable->import_css) return;

        litehtml::string result;
        litehtml::string* result_ptr = &result;

        vtable->import_css(
            user_data,
            url.c_str(),
            baseurl.c_str(),
            [](void* ctx, const char* r) {
                auto* p = static_cast<litehtml::string*>(ctx);
                *p = r ? r : "";
            },
            result_ptr);

        text = result;
    }

    /* -- set_clip -- */
    void set_clip(const litehtml::position& pos,
                  const litehtml::border_radiuses& bdr_radius) override
    {
        if (vtable->set_clip)
            vtable->set_clip(user_data, to_c(pos), to_c(bdr_radius));
    }

    /* -- del_clip -- */
    void del_clip() override
    {
        if (vtable->del_clip)
            vtable->del_clip(user_data);
    }

    /* -- get_viewport -- */
    void get_viewport(litehtml::position& viewport) const override
    {
        if (!vtable->get_viewport) return;
        lh_position_t c_vp = to_c(viewport);
        vtable->get_viewport(user_data, &c_vp);
        viewport = to_cpp(c_vp);
    }

    /* -- create_element -- */
    litehtml::element::ptr create_element(
        const char* /*tag_name*/,
        const litehtml::string_map& /*attributes*/,
        const std::shared_ptr<litehtml::document>& /*doc*/) override
    {
        /* Return null so litehtml creates the default element. */
        return nullptr;
    }

    /* -- get_media_features -- */
    void get_media_features(litehtml::media_features& media) const override
    {
        if (!vtable->get_media_features) return;
        lh_media_features_t c_mf = to_c(media);
        vtable->get_media_features(user_data, &c_mf);
        from_c(c_mf, media);
    }

    /* -- get_language -- */
    void get_language(litehtml::string& language,
                      litehtml::string& culture) const override
    {
        if (!vtable->get_language) return;

        struct LangCtx {
            litehtml::string* lang;
            litehtml::string* cult;
        } ctx = {&language, &culture};

        vtable->get_language(
            user_data,
            [](void* c, const char* lang, const char* cult) {
                auto* lc = static_cast<LangCtx*>(c);
                *lc->lang = lang ? lang : "";
                *lc->cult = cult ? cult : "";
            },
            &ctx);
    }
};

/* --------------------------------------------------------------------------
 * Accessor functions -- font_description
 * -------------------------------------------------------------------------- */

extern "C" {

const char* lh_font_description_family(const lh_font_description_t* fd)
{
    if (!fd) return "";
    const auto* d = reinterpret_cast<const litehtml::font_description*>(fd);
    return d->family.c_str();
}

float lh_font_description_size(const lh_font_description_t* fd)
{
    if (!fd) return 0.0f;
    const auto* d = reinterpret_cast<const litehtml::font_description*>(fd);
    return d->size;
}

int lh_font_description_style(const lh_font_description_t* fd)
{
    if (!fd) return 0;
    const auto* d = reinterpret_cast<const litehtml::font_description*>(fd);
    return static_cast<int>(d->style);
}

int lh_font_description_weight(const lh_font_description_t* fd)
{
    if (!fd) return 0;
    const auto* d = reinterpret_cast<const litehtml::font_description*>(fd);
    return d->weight;
}

int lh_font_description_decoration_line(const lh_font_description_t* fd)
{
    if (!fd) return 0;
    const auto* d = reinterpret_cast<const litehtml::font_description*>(fd);
    return d->decoration_line;
}

int lh_font_description_decoration_thickness_is_predefined(const lh_font_description_t* fd)
{
    if (!fd) return 1;
    const auto* d = reinterpret_cast<const litehtml::font_description*>(fd);
    return d->decoration_thickness.is_predefined() ? 1 : 0;
}

int lh_font_description_decoration_thickness_predef(const lh_font_description_t* fd)
{
    if (!fd) return 0;
    const auto* d = reinterpret_cast<const litehtml::font_description*>(fd);
    return d->decoration_thickness.is_predefined() ? d->decoration_thickness.predef() : 0;
}

float lh_font_description_decoration_thickness_value(const lh_font_description_t* fd)
{
    if (!fd) return 0.0f;
    const auto* d = reinterpret_cast<const litehtml::font_description*>(fd);
    return d->decoration_thickness.is_predefined() ? 0.0f : d->decoration_thickness.val();
}

int lh_font_description_decoration_style(const lh_font_description_t* fd)
{
    if (!fd) return 0;
    const auto* d = reinterpret_cast<const litehtml::font_description*>(fd);
    return static_cast<int>(d->decoration_style);
}

lh_web_color_t lh_font_description_decoration_color(const lh_font_description_t* fd)
{
    if (!fd) return {};
    const auto* d = reinterpret_cast<const litehtml::font_description*>(fd);
    return to_c(d->decoration_color);
}

const char* lh_font_description_emphasis_style(const lh_font_description_t* fd)
{
    if (!fd) return "";
    const auto* d = reinterpret_cast<const litehtml::font_description*>(fd);
    return d->emphasis_style.c_str();
}

lh_web_color_t lh_font_description_emphasis_color(const lh_font_description_t* fd)
{
    if (!fd) return {};
    const auto* d = reinterpret_cast<const litehtml::font_description*>(fd);
    return to_c(d->emphasis_color);
}

int lh_font_description_emphasis_position(const lh_font_description_t* fd)
{
    if (!fd) return 0;
    const auto* d = reinterpret_cast<const litehtml::font_description*>(fd);
    return d->emphasis_position;
}

/* --------------------------------------------------------------------------
 * Accessor functions -- list_marker
 * -------------------------------------------------------------------------- */

const char* lh_list_marker_image(const lh_list_marker_t* m)
{
    if (!m) return "";
    const auto* mk = reinterpret_cast<const litehtml::list_marker*>(m);
    return mk->image.c_str();
}

const char* lh_list_marker_baseurl(const lh_list_marker_t* m)
{
    if (!m) return "";
    const auto* mk = reinterpret_cast<const litehtml::list_marker*>(m);
    return mk->baseurl;
}

int lh_list_marker_type(const lh_list_marker_t* m)
{
    if (!m) return 0;
    const auto* mk = reinterpret_cast<const litehtml::list_marker*>(m);
    return static_cast<int>(mk->marker_type);
}

lh_web_color_t lh_list_marker_color(const lh_list_marker_t* m)
{
    if (!m) return {};
    const auto* mk = reinterpret_cast<const litehtml::list_marker*>(m);
    return to_c(mk->color);
}

lh_position_t lh_list_marker_pos(const lh_list_marker_t* m)
{
    if (!m) return {};
    const auto* mk = reinterpret_cast<const litehtml::list_marker*>(m);
    return to_c(mk->pos);
}

int lh_list_marker_index(const lh_list_marker_t* m)
{
    if (!m) return 0;
    const auto* mk = reinterpret_cast<const litehtml::list_marker*>(m);
    return mk->index;
}

uintptr_t lh_list_marker_font(const lh_list_marker_t* m)
{
    if (!m) return 0;
    const auto* mk = reinterpret_cast<const litehtml::list_marker*>(m);
    return mk->font;
}

/* --------------------------------------------------------------------------
 * Accessor functions -- background_layer
 * -------------------------------------------------------------------------- */

lh_position_t lh_background_layer_border_box(const lh_background_layer_t* layer)
{
    if (!layer) return {};
    const auto* bl = reinterpret_cast<const litehtml::background_layer*>(layer);
    return to_c(bl->border_box);
}

lh_border_radiuses_t lh_background_layer_border_radius(const lh_background_layer_t* layer)
{
    if (!layer) return {};
    const auto* bl = reinterpret_cast<const litehtml::background_layer*>(layer);
    return to_c(bl->border_radius);
}

lh_position_t lh_background_layer_clip_box(const lh_background_layer_t* layer)
{
    if (!layer) return {};
    const auto* bl = reinterpret_cast<const litehtml::background_layer*>(layer);
    return to_c(bl->clip_box);
}

lh_position_t lh_background_layer_origin_box(const lh_background_layer_t* layer)
{
    if (!layer) return {};
    const auto* bl = reinterpret_cast<const litehtml::background_layer*>(layer);
    return to_c(bl->origin_box);
}

int lh_background_layer_attachment(const lh_background_layer_t* layer)
{
    if (!layer) return 0;
    const auto* bl = reinterpret_cast<const litehtml::background_layer*>(layer);
    return static_cast<int>(bl->attachment);
}

int lh_background_layer_repeat(const lh_background_layer_t* layer)
{
    if (!layer) return 0;
    const auto* bl = reinterpret_cast<const litehtml::background_layer*>(layer);
    return static_cast<int>(bl->repeat);
}

int lh_background_layer_is_root(const lh_background_layer_t* layer)
{
    if (!layer) return 0;
    const auto* bl = reinterpret_cast<const litehtml::background_layer*>(layer);
    return bl->is_root ? 1 : 0;
}

/* --------------------------------------------------------------------------
 * Accessor functions -- linear_gradient
 * -------------------------------------------------------------------------- */

lh_point_t lh_linear_gradient_start(const lh_linear_gradient_t* g)
{
    if (!g) return {};
    const auto* lg = reinterpret_cast<
        const litehtml::background_layer::linear_gradient*>(g);
    return to_c(lg->start);
}

lh_point_t lh_linear_gradient_end(const lh_linear_gradient_t* g)
{
    if (!g) return {};
    const auto* lg = reinterpret_cast<
        const litehtml::background_layer::linear_gradient*>(g);
    return to_c(lg->end);
}

int lh_linear_gradient_color_points_count(const lh_linear_gradient_t* g)
{
    if (!g) return 0;
    const auto* lg = reinterpret_cast<
        const litehtml::background_layer::linear_gradient*>(g);
    return static_cast<int>(lg->color_points.size());
}

float lh_linear_gradient_color_point_offset(const lh_linear_gradient_t* g,
                                             int idx)
{
    if (!g) return 0.0f;
    const auto* lg = reinterpret_cast<
        const litehtml::background_layer::linear_gradient*>(g);
    if (idx < 0 || idx >= static_cast<int>(lg->color_points.size()))
        return 0.0f;
    return lg->color_points[idx].offset;
}

lh_web_color_t lh_linear_gradient_color_point_color(
    const lh_linear_gradient_t* g, int idx)
{
    if (!g) return {};
    const auto* lg = reinterpret_cast<
        const litehtml::background_layer::linear_gradient*>(g);
    if (idx < 0 || idx >= static_cast<int>(lg->color_points.size()))
    {
        lh_web_color_t zero = {};
        return zero;
    }
    return to_c(lg->color_points[idx].color);
}

int lh_linear_gradient_color_space(const lh_linear_gradient_t* g)
{
    if (!g) return 0;
    const auto* lg = reinterpret_cast<
        const litehtml::background_layer::linear_gradient*>(g);
    return static_cast<int>(lg->color_space);
}

int lh_linear_gradient_hue_interpolation(const lh_linear_gradient_t* g)
{
    if (!g) return 0;
    const auto* lg = reinterpret_cast<
        const litehtml::background_layer::linear_gradient*>(g);
    return static_cast<int>(lg->hue_interpolation);
}

/* --------------------------------------------------------------------------
 * Accessor functions -- radial_gradient
 * -------------------------------------------------------------------------- */

lh_point_t lh_radial_gradient_position(const lh_radial_gradient_t* g)
{
    if (!g) return {};
    const auto* rg = reinterpret_cast<
        const litehtml::background_layer::radial_gradient*>(g);
    return to_c(rg->position);
}

lh_point_t lh_radial_gradient_radius(const lh_radial_gradient_t* g)
{
    if (!g) return {};
    const auto* rg = reinterpret_cast<
        const litehtml::background_layer::radial_gradient*>(g);
    return to_c(rg->radius);
}

int lh_radial_gradient_color_points_count(const lh_radial_gradient_t* g)
{
    if (!g) return 0;
    const auto* rg = reinterpret_cast<
        const litehtml::background_layer::radial_gradient*>(g);
    return static_cast<int>(rg->color_points.size());
}

float lh_radial_gradient_color_point_offset(const lh_radial_gradient_t* g,
                                             int idx)
{
    if (!g) return 0.0f;
    const auto* rg = reinterpret_cast<
        const litehtml::background_layer::radial_gradient*>(g);
    if (idx < 0 || idx >= static_cast<int>(rg->color_points.size()))
        return 0.0f;
    return rg->color_points[idx].offset;
}

lh_web_color_t lh_radial_gradient_color_point_color(
    const lh_radial_gradient_t* g, int idx)
{
    if (!g) return {};
    const auto* rg = reinterpret_cast<
        const litehtml::background_layer::radial_gradient*>(g);
    if (idx < 0 || idx >= static_cast<int>(rg->color_points.size()))
    {
        lh_web_color_t zero = {};
        return zero;
    }
    return to_c(rg->color_points[idx].color);
}

int lh_radial_gradient_color_space(const lh_radial_gradient_t* g)
{
    if (!g) return 0;
    const auto* rg = reinterpret_cast<
        const litehtml::background_layer::radial_gradient*>(g);
    return static_cast<int>(rg->color_space);
}

int lh_radial_gradient_hue_interpolation(const lh_radial_gradient_t* g)
{
    if (!g) return 0;
    const auto* rg = reinterpret_cast<
        const litehtml::background_layer::radial_gradient*>(g);
    return static_cast<int>(rg->hue_interpolation);
}

/* --------------------------------------------------------------------------
 * Accessor functions -- conic_gradient
 * -------------------------------------------------------------------------- */

lh_point_t lh_conic_gradient_position(const lh_conic_gradient_t* g)
{
    if (!g) return {};
    const auto* cg = reinterpret_cast<
        const litehtml::background_layer::conic_gradient*>(g);
    return to_c(cg->position);
}

float lh_conic_gradient_angle(const lh_conic_gradient_t* g)
{
    if (!g) return 0.0f;
    const auto* cg = reinterpret_cast<
        const litehtml::background_layer::conic_gradient*>(g);
    return cg->angle;
}

float lh_conic_gradient_radius(const lh_conic_gradient_t* g)
{
    if (!g) return 0.0f;
    const auto* cg = reinterpret_cast<
        const litehtml::background_layer::conic_gradient*>(g);
    return cg->radius;
}

int lh_conic_gradient_color_points_count(const lh_conic_gradient_t* g)
{
    if (!g) return 0;
    const auto* cg = reinterpret_cast<
        const litehtml::background_layer::conic_gradient*>(g);
    return static_cast<int>(cg->color_points.size());
}

float lh_conic_gradient_color_point_offset(const lh_conic_gradient_t* g,
                                            int idx)
{
    if (!g) return 0.0f;
    const auto* cg = reinterpret_cast<
        const litehtml::background_layer::conic_gradient*>(g);
    if (idx < 0 || idx >= static_cast<int>(cg->color_points.size()))
        return 0.0f;
    return cg->color_points[idx].offset;
}

lh_web_color_t lh_conic_gradient_color_point_color(
    const lh_conic_gradient_t* g, int idx)
{
    if (!g) return {};
    const auto* cg = reinterpret_cast<
        const litehtml::background_layer::conic_gradient*>(g);
    if (idx < 0 || idx >= static_cast<int>(cg->color_points.size()))
    {
        lh_web_color_t zero = {};
        return zero;
    }
    return to_c(cg->color_points[idx].color);
}

int lh_conic_gradient_color_space(const lh_conic_gradient_t* g)
{
    if (!g) return 0;
    const auto* cg = reinterpret_cast<
        const litehtml::background_layer::conic_gradient*>(g);
    return static_cast<int>(cg->color_space);
}

int lh_conic_gradient_hue_interpolation(const lh_conic_gradient_t* g)
{
    if (!g) return 0;
    const auto* cg = reinterpret_cast<
        const litehtml::background_layer::conic_gradient*>(g);
    return static_cast<int>(cg->hue_interpolation);
}

/* --------------------------------------------------------------------------
 * Document lifecycle
 * -------------------------------------------------------------------------- */

lh_document_t* lh_document_create_from_string(
    const char* html,
    lh_container_vtable_t* vtable,
    void* user_data,
    const char* master_css,
    const char* user_styles)
{
    if (!html || !vtable) return nullptr;

    auto* container = new CDocumentContainer(vtable, user_data);

    std::string master = master_css ? master_css : litehtml::master_css;
    std::string user   = user_styles ? user_styles : "";

    litehtml::document::ptr doc =
        litehtml::document::createFromString(html, container, master, user);

    if (!doc) {
        delete container;
        return nullptr;
    }

    auto* internal    = new lh_document_internal;
    internal->doc       = doc;
    internal->container = container;

    return reinterpret_cast<lh_document_t*>(internal);
}

void lh_document_destroy(lh_document_t* doc)
{
    if (!doc) return;
    auto* internal = reinterpret_cast<lh_document_internal*>(doc);
    /* Drop the document (shared_ptr) first -- its destructor calls back into
       the container (e.g. delete_font), so the container must still be alive. */
    auto* container = internal->container;
    delete internal;
    delete container;
}

float lh_document_render(lh_document_t* doc, float max_width)
{
    if (!doc) return 0;
    auto* internal = reinterpret_cast<lh_document_internal*>(doc);
    return internal->doc->render(max_width);
}

void lh_document_draw(lh_document_t* doc,
                       uintptr_t hdc,
                       float x, float y,
                       const lh_position_t* clip)
{
    if (!doc) return;
    auto* internal = reinterpret_cast<lh_document_internal*>(doc);

    if (clip) {
        litehtml::position cpp_clip = to_cpp(*clip);
        internal->doc->draw(hdc, x, y, &cpp_clip);
    } else {
        internal->doc->draw(hdc, x, y, nullptr);
    }
}

float lh_document_width(const lh_document_t* doc)
{
    if (!doc) return 0;
    const auto* internal =
        reinterpret_cast<const lh_document_internal*>(doc);
    return internal->doc->width();
}

float lh_document_height(const lh_document_t* doc)
{
    if (!doc) return 0;
    const auto* internal =
        reinterpret_cast<const lh_document_internal*>(doc);
    return internal->doc->height();
}

/* --------------------------------------------------------------------------
 * Document content manipulation
 * -------------------------------------------------------------------------- */

void lh_document_add_stylesheet(lh_document_t* doc,
                                const char* css_text,
                                const char* baseurl,
                                const char* media)
{
    if (!doc || !css_text || !css_text[0]) return;
    auto* internal = reinterpret_cast<lh_document_internal*>(doc);

    litehtml::css stylesheet;
    litehtml::media_query_list_list::ptr mq;
    if (media && media[0]) {
        auto mq_list = litehtml::parse_media_query_list(media, internal->doc);
        mq = std::make_shared<litehtml::media_query_list_list>();
        mq->add(mq_list);
    }
    stylesheet.parse_css_stylesheet(
        css_text,
        baseurl ? baseurl : "",
        internal->doc,
        mq);
    stylesheet.sort_selectors();

    auto root = internal->doc->root();
    if (root) {
        root->apply_stylesheet(stylesheet);
        root->compute_styles();
    }
}

lh_element_t* lh_document_root(lh_document_t* doc)
{
    if (!doc) return nullptr;
    auto* internal = reinterpret_cast<lh_document_internal*>(doc);
    auto root = internal->doc->root();
    return reinterpret_cast<lh_element_t*>(root.get());
}

void lh_document_append_children_from_string(lh_document_t* doc,
                                              lh_element_t* parent,
                                              const char* html,
                                              int replace_existing)
{
    if (!doc || !parent || !html) return;
    auto* internal = reinterpret_cast<lh_document_internal*>(doc);
    auto* elem = reinterpret_cast<litehtml::element*>(parent);
    internal->doc->append_children_from_string(*elem, html, replace_existing != 0);
}

/* --------------------------------------------------------------------------
 * Mouse / interaction
 * -------------------------------------------------------------------------- */

int lh_document_on_mouse_over(lh_document_t* doc,
                               float x, float y,
                               float client_x, float client_y)
{
    if (!doc) return 0;
    auto* internal = reinterpret_cast<lh_document_internal*>(doc);
    litehtml::position::vector redraw_boxes;
    bool changed = internal->doc->on_mouse_over(x, y, client_x, client_y,
                                                 redraw_boxes);
    return changed ? 1 : 0;
}

int lh_document_on_lbutton_down(lh_document_t* doc,
                                 float x, float y,
                                 float client_x, float client_y)
{
    if (!doc) return 0;
    auto* internal = reinterpret_cast<lh_document_internal*>(doc);
    litehtml::position::vector redraw_boxes;
    bool changed = internal->doc->on_lbutton_down(x, y, client_x, client_y,
                                                   redraw_boxes);
    return changed ? 1 : 0;
}

int lh_document_on_lbutton_up(lh_document_t* doc,
                               float x, float y,
                               float client_x, float client_y)
{
    if (!doc) return 0;
    auto* internal = reinterpret_cast<lh_document_internal*>(doc);
    litehtml::position::vector redraw_boxes;
    bool changed = internal->doc->on_lbutton_up(x, y, client_x, client_y,
                                                 redraw_boxes);
    return changed ? 1 : 0;
}

int lh_document_on_mouse_leave(lh_document_t* doc)
{
    if (!doc) return 0;
    auto* internal = reinterpret_cast<lh_document_internal*>(doc);
    litehtml::position::vector redraw_boxes;
    bool changed = internal->doc->on_mouse_leave(redraw_boxes);
    return changed ? 1 : 0;
}

int lh_document_media_changed(lh_document_t* doc)
{
    if (!doc) return 0;
    auto* internal = reinterpret_cast<lh_document_internal*>(doc);
    return internal->doc->media_changed() ? 1 : 0;
}

/* --------------------------------------------------------------------------
 * Element introspection
 * -------------------------------------------------------------------------- */

lh_element_t* lh_element_parent(lh_element_t* el)
{
    if (!el) return nullptr;
    auto* elem = reinterpret_cast<litehtml::element*>(el);
    auto parent = elem->parent();
    if (!parent) return nullptr;
    return reinterpret_cast<lh_element_t*>(parent.get());
}

int lh_element_children_count(lh_element_t* el)
{
    if (!el) return 0;
    auto* elem = reinterpret_cast<litehtml::element*>(el);
    return static_cast<int>(elem->children().size());
}

lh_element_t* lh_element_child_at(lh_element_t* el, int index)
{
    if (!el) return nullptr;
    auto* elem = reinterpret_cast<litehtml::element*>(el);
    const auto& kids = elem->children();
    if (index < 0 || index >= static_cast<int>(kids.size()))
        return nullptr;
    auto it = kids.begin();
    std::advance(it, index);
    return reinterpret_cast<lh_element_t*>(it->get());
}

int lh_element_is_text(lh_element_t* el)
{
    if (!el) return 0;
    auto* elem = reinterpret_cast<litehtml::element*>(el);
    return elem->is_text() ? 1 : 0;
}

uintptr_t lh_element_get_font(lh_element_t* el)
{
    if (!el) return 0;
    auto* elem = reinterpret_cast<litehtml::element*>(el);
    return elem->css().get_font();
}

float lh_element_get_font_size(lh_element_t* el)
{
    if (!el) return 0.0f;
    auto* elem = reinterpret_cast<litehtml::element*>(el);
    return elem->css().get_font_size();
}

void lh_element_get_placement(lh_element_t* el, lh_position_t* pos)
{
    if (!el || !pos) return;
    auto* elem = reinterpret_cast<litehtml::element*>(el);
    litehtml::position p = elem->get_placement();
    *pos = to_c(p);
}

void lh_element_get_text(lh_element_t* el,
                         void (*cb)(void* ctx, const char* text),
                         void* ctx)
{
    if (!el || !cb) return;
    auto* elem = reinterpret_cast<litehtml::element*>(el);
    litehtml::string text;
    elem->get_text(text);
    cb(ctx, text.c_str());
}

lh_element_t* lh_document_get_element_by_point(lh_document_t* doc,
                                                float x, float y,
                                                float client_x, float client_y)
{
    if (!doc) return nullptr;
    auto* internal = reinterpret_cast<lh_document_internal*>(doc);
    auto root_render = internal->doc->root_render();
    if (!root_render) return nullptr;
    auto el = root_render->get_element_by_point(x, y, client_x, client_y,
        [](const std::shared_ptr<litehtml::render_item>&) { return true; });
    if (!el) return nullptr;
    return reinterpret_cast<lh_element_t*>(el.get());
}

/* --------------------------------------------------------------------------
 * Inline box helpers
 *
 * get_inline_boxes() returns local-coordinate boxes from the render item.
 * We compute the same parent-chain offset that get_placement() uses, then
 * apply it to each box so callers get absolute document coordinates.
 * -------------------------------------------------------------------------- */

/* Compute parent-chain offset: placement.{x,y} - m_pos.{x,y} */
static void compute_ri_offset(const std::shared_ptr<litehtml::render_item>& ri,
                               float& ox, float& oy)
{
    litehtml::position placement = ri->get_placement();
    litehtml::position pos = ri->pos();
    ox = placement.x - pos.x;
    oy = placement.y - pos.y;
}

int lh_element_get_inline_boxes_count(lh_element_t* el)
{
    if (!el) return 0;
    auto* elem = reinterpret_cast<litehtml::element*>(el);
    auto ri = elem->get_render_item();
    if (!ri) return 0;
    litehtml::position::vector boxes;
    ri->get_inline_boxes(boxes);
    return static_cast<int>(boxes.size());
}

void lh_element_get_inline_box_at(lh_element_t* el, int index, lh_position_t* pos)
{
    if (!el || !pos) return;
    auto* elem = reinterpret_cast<litehtml::element*>(el);
    auto ri = elem->get_render_item();
    if (!ri) return;

    litehtml::position::vector boxes;
    ri->get_inline_boxes(boxes);
    if (index < 0 || index >= static_cast<int>(boxes.size()))
        return;

    float ox, oy;
    compute_ri_offset(ri, ox, oy);

    litehtml::position box = boxes[index];
    box.x += ox;
    box.y += oy;
    *pos = to_c(box);
}

int lh_element_get_text_align(lh_element_t* el)
{
    if (!el) return 0;
    auto* elem = reinterpret_cast<litehtml::element*>(el);
    return static_cast<int>(elem->css().get_text_align());
}

float lh_element_get_line_height(lh_element_t* el)
{
    if (!el) return 0.0f;
    auto* elem = reinterpret_cast<litehtml::element*>(el);
    return elem->css().line_height().computed_value;
}

} /* extern "C" */
