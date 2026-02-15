use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let csrc_dir = manifest_dir.join("csrc");

    let vendored = env::var("CARGO_FEATURE_VENDORED").is_ok();

    if vendored {
        build_vendored(&manifest_dir, &out_dir, &csrc_dir);
    } else {
        build_system(&out_dir, &csrc_dir);
    }

    generate_bindings(&csrc_dir, &out_dir, vendored, &manifest_dir);

    println!("cargo:rerun-if-changed={}", csrc_dir.display());
}

/// Build from vendored C++ source in vendor/litehtml
fn build_vendored(manifest_dir: &PathBuf, out_dir: &PathBuf, csrc_dir: &PathBuf) {
    let vendor_dir = manifest_dir.join("vendor/litehtml");
    let gumbo_src = vendor_dir.join("src/gumbo");
    let gumbo_include = gumbo_src.join("include");
    let gumbo_private_include = gumbo_include.join("gumbo");
    let litehtml_src = vendor_dir.join("src");
    let litehtml_include = vendor_dir.join("include");

    // Gumbo (C99)
    cc::Build::new()
        .cargo_metadata(false)
        .files(
            [
                "attribute.c",
                "char_ref.c",
                "error.c",
                "parser.c",
                "string_buffer.c",
                "string_piece.c",
                "tag.c",
                "tokenizer.c",
                "utf8.c",
                "util.c",
                "vector.c",
            ]
            .iter()
            .map(|f| gumbo_src.join(f)),
        )
        .include(&gumbo_include)
        .include(&gumbo_private_include)
        .std("c99")
        .warnings(false)
        .compile("gumbo");

    // litehtml (C++17)
    cc::Build::new()
        .cargo_metadata(false)
        .cpp(true)
        .files(
            [
                "codepoint.cpp",
                "css_length.cpp",
                "css_selector.cpp",
                "css_tokenizer.cpp",
                "css_parser.cpp",
                "document.cpp",
                "document_container.cpp",
                "el_anchor.cpp",
                "el_base.cpp",
                "el_before_after.cpp",
                "el_body.cpp",
                "el_break.cpp",
                "el_cdata.cpp",
                "el_comment.cpp",
                "el_div.cpp",
                "element.cpp",
                "el_font.cpp",
                "el_image.cpp",
                "el_link.cpp",
                "el_para.cpp",
                "el_script.cpp",
                "el_space.cpp",
                "el_style.cpp",
                "el_table.cpp",
                "el_td.cpp",
                "el_text.cpp",
                "el_title.cpp",
                "el_tr.cpp",
                "encodings.cpp",
                "html.cpp",
                "html_tag.cpp",
                "html_microsyntaxes.cpp",
                "iterators.cpp",
                "media_query.cpp",
                "style.cpp",
                "stylesheet.cpp",
                "table.cpp",
                "tstring_view.cpp",
                "url.cpp",
                "url_path.cpp",
                "utf8_strings.cpp",
                "web_color.cpp",
                "num_cvt.cpp",
                "strtod.cpp",
                "string_id.cpp",
                "css_properties.cpp",
                "line_box.cpp",
                "css_borders.cpp",
                "render_item.cpp",
                "render_block_context.cpp",
                "render_block.cpp",
                "render_inline_context.cpp",
                "render_table.cpp",
                "render_flex.cpp",
                "render_image.cpp",
                "formatting_context.cpp",
                "flex_item.cpp",
                "flex_line.cpp",
                "background.cpp",
                "gradient.cpp",
            ]
            .iter()
            .map(|f| litehtml_src.join(f)),
        )
        .include(&litehtml_include)
        .include(litehtml_include.join("litehtml"))
        .include(&litehtml_src)
        .include(&gumbo_include)
        .std("c++17")
        .warnings(false)
        .compile("litehtml");

    // C wrapper (C++17)
    cc::Build::new()
        .cargo_metadata(false)
        .cpp(true)
        .file(csrc_dir.join("litehtml_c.cpp"))
        .include(&litehtml_include)
        .include(&gumbo_include)
        .std("c++17")
        .warnings(false)
        .compile("litehtml_c");

    // Link order: dependents first
    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=litehtml_c");
    println!("cargo:rustc-link-lib=static=litehtml");
    println!("cargo:rustc-link-lib=static=gumbo");
    println!("cargo:rustc-link-lib=stdc++");

    println!("cargo:rerun-if-changed={}", vendor_dir.display());
}

/// Build against system-installed litehtml
fn build_system(out_dir: &PathBuf, csrc_dir: &PathBuf) {
    // Find system litehtml via LITEHTML_DIR or common paths
    let litehtml_dir = env::var("LITEHTML_DIR").ok().map(PathBuf::from);

    let (include_dir, lib_dir) = if let Some(ref dir) = litehtml_dir {
        (dir.join("include"), dir.join("lib"))
    } else {
        // Search LIBRARY_PATH / C_INCLUDE_PATH (set by GUIX, Nix, etc.)
        let include_dir = find_header("litehtml.h").expect(
            "Cannot find litehtml headers. Set LITEHTML_DIR or enable the `vendored` feature.",
        );
        let lib_dir = find_library("liblitehtml.a")
            .expect("Cannot find liblitehtml. Set LITEHTML_DIR or enable the `vendored` feature.");
        (include_dir, lib_dir)
    };

    // Compile just the C wrapper against system headers
    // litehtml installs headers under include/litehtml/, and the C wrapper
    // does #include <litehtml.h>, so we need include/litehtml/ on the path.
    let litehtml_subdir = include_dir.join("litehtml");
    let effective_include = if litehtml_subdir.join("litehtml.h").exists() {
        &litehtml_subdir
    } else {
        &include_dir
    };

    cc::Build::new()
        .cargo_metadata(false)
        .cpp(true)
        .file(csrc_dir.join("litehtml_c.cpp"))
        .include(effective_include)
        .std("c++17")
        .warnings(false)
        .compile("litehtml_c");

    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=static=litehtml_c");
    println!("cargo:rustc-link-lib=static=litehtml");
    println!("cargo:rustc-link-lib=stdc++");
}

/// Search include paths for a header file, return the directory containing it
fn find_header(name: &str) -> Option<PathBuf> {
    // Check C_INCLUDE_PATH and CPLUS_INCLUDE_PATH
    for var in &["C_INCLUDE_PATH", "CPLUS_INCLUDE_PATH"] {
        if let Ok(paths) = env::var(var) {
            for path in paths.split(':') {
                let candidate = PathBuf::from(path);
                if candidate.join("litehtml").join(name).exists() {
                    return Some(candidate);
                }
            }
        }
    }
    // Check common system paths
    for prefix in &["/usr", "/usr/local"] {
        let candidate = PathBuf::from(prefix).join("include");
        if candidate.join("litehtml").join(name).exists() {
            return Some(candidate);
        }
    }
    None
}

/// Search library paths for a library file, return the directory containing it
fn find_library(name: &str) -> Option<PathBuf> {
    if let Ok(paths) = env::var("LIBRARY_PATH") {
        for path in paths.split(':') {
            let candidate = PathBuf::from(path);
            if candidate.join(name).exists() {
                return Some(candidate);
            }
        }
    }
    for prefix in &["/usr/lib", "/usr/local/lib", "/usr/lib/x86_64-linux-gnu"] {
        let candidate = PathBuf::from(prefix);
        if candidate.join(name).exists() {
            return Some(candidate);
        }
    }
    None
}

fn generate_bindings(
    csrc_dir: &PathBuf,
    out_dir: &PathBuf,
    vendored: bool,
    manifest_dir: &PathBuf,
) {
    let mut builder = bindgen::Builder::default()
        .header(csrc_dir.join("litehtml_c.h").to_str().unwrap())
        .allowlist_function("lh_.*")
        .allowlist_type("lh_.*")
        .derive_debug(true)
        .derive_default(true);

    if vendored {
        let vendor_dir = manifest_dir.join("vendor/litehtml");
        let litehtml_include = vendor_dir.join("include");
        let gumbo_include = vendor_dir.join("src/gumbo/include");
        builder = builder
            .clang_arg(format!("-I{}", litehtml_include.to_str().unwrap()))
            .clang_arg(format!("-I{}", gumbo_include.to_str().unwrap()));
    }

    let bindings = builder.generate().expect("failed to generate bindings");

    bindings
        .write_to_file(out_dir.join("bindings.rs"))
        .expect("failed to write bindings");
}
