//! Copies the `tools/profile-gui` files that `src/gui.rs` embeds into `OUT_DIR`, minified when the `desktop`
//! feature (the only build that serves them, through `ssgg-gui`) is on. A file that fails to parse fails the
//! build instead of shipping unminified.

use std::path::{Path, PathBuf};

const GUI_DIR: &str = "tools/profile-gui";
const GUI_FILES: &[&str] = &[
    "index.html",
    "app.js",
    "control.js",
    "headset.js",
    "live.js",
    "main.js",
    "products.generated.js",
    "style.css",
];

fn main() {
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"));
    for name in GUI_FILES {
        let path = Path::new(GUI_DIR).join(name);
        println!("cargo::rerun-if-changed={}", path.display());
        let source = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        let output = minify(name, &source).unwrap_or_else(|e| panic!("minify {}: {e}", path.display()));
        let target = out_dir.join(name);
        std::fs::write(&target, output).unwrap_or_else(|e| panic!("write {}: {e}", target.display()));
    }
}

#[cfg(not(feature = "desktop"))]
fn minify(_name: &str, source: &str) -> Result<String, String> {
    Ok(source.to_owned())
}

#[cfg(feature = "desktop")]
fn minify(name: &str, source: &str) -> Result<String, String> {
    match Path::new(name).extension().and_then(|e| e.to_str()) {
        Some("js") => minify_js(source),
        Some("css") => minify_css(source),
        Some("html") => minify_html(source),
        _ => Err("no minifier for this file type".to_owned()),
    }
}

#[cfg(feature = "desktop")]
fn minify_js(source: &str) -> Result<String, String> {
    use oxc_allocator::Allocator;
    use oxc_codegen::{Codegen, CodegenOptions};
    use oxc_minifier::{CompressOptions, MangleOptions, Minifier, MinifierOptions};
    use oxc_parser::Parser;
    use oxc_span::SourceType;

    let allocator = Allocator::default();
    // Classic `<script defer>` files share one global scope, so they parse as scripts: top-level names stay
    // as written (`MangleOptions::top_level` is off) and other files can still call them.
    let parsed = Parser::new(&allocator, source, SourceType::cjs()).parse();
    if let Some(error) = parsed.errors.first() {
        return Err(error.to_string());
    }
    let mut program = parsed.program;
    let minified = Minifier::new(MinifierOptions {
        mangle: Some(MangleOptions::default()),
        compress: Some(CompressOptions::safest()),
    })
    .minify(&allocator, &mut program);
    Ok(Codegen::new()
        .with_options(CodegenOptions::minify())
        .with_scoping(minified.scoping)
        .build(&program)
        .code)
}

#[cfg(feature = "desktop")]
fn minify_css(source: &str) -> Result<String, String> {
    use lightningcss::stylesheet::{MinifyOptions, ParserOptions, PrinterOptions, StyleSheet};

    let mut sheet = StyleSheet::parse(source, ParserOptions::default()).map_err(|e| e.to_string())?;
    sheet.minify(MinifyOptions::default()).map_err(|e| e.to_string())?;
    let printed = sheet
        .to_css(PrinterOptions {
            minify: true,
            ..PrinterOptions::default()
        })
        .map_err(|e| e.to_string())?;
    Ok(printed.code)
}

#[cfg(feature = "desktop")]
fn minify_html(source: &str) -> Result<String, String> {
    let cfg = minify_html::Cfg {
        // `src/gui.rs` puts the session token in front of `</head>`.
        keep_closing_tags: true,
        keep_html_and_head_opening_tags: true,
        ..minify_html::Cfg::default()
    };
    String::from_utf8(minify_html::minify(source.as_bytes(), &cfg)).map_err(|e| e.to_string())
}
