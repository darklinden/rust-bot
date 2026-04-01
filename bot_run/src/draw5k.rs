use crate::feature::{Feature, MessageContext};
use async_trait::async_trait;
use base64::Engine;
use bot_lib::structs::{MessageSegment, Segment};
use serde_json::Value;

pub struct Draw5kFeature;

static UPPER_FONT_DATA: &[u8] = include_bytes!("../assets/fonts/SourceHanSerif-Heavy.otf");
static LOWER_FONT_DATA: &[u8] = include_bytes!("../assets/fonts/SourceHanSans-Heavy.otf");

const UPPER_FONT_FAMILY: &str = "Source Han Serif CN";
const LOWER_FONT_FAMILY: &str = "Source Han Sans CN";

const MAX_LENGTH: usize = 42;
const DEFAULT_OFFSET_X: i32 = 200;

fn trim_both_ends(s: &str) -> String {
    let mut result = s.trim().to_string();
    while (result.starts_with('"') && result.ends_with('"'))
        || (result.starts_with('\'') && result.ends_with('\''))
    {
        if (result.starts_with('"') && result.ends_with('"'))
            || (result.starts_with('\'') && result.ends_with('\''))
        {
            result = result[1..result.len() - 1].trim().to_string();
        } else {
            break;
        }
    }
    result
}

fn parse_args(args: &str) -> (String, String) {
    let s = args.trim();
    if s.is_empty() {
        return (String::new(), String::new());
    }

    if s.starts_with('"') || s.starts_with('\'') {
        let quote = if s.starts_with('"') { '"' } else { '\'' };

        if let Some(close_pos) = s[1..].find(quote) {
            let upper = trim_both_ends(&s[1..close_pos + 1]);
            let lower = trim_both_ends(&s[close_pos + 2..]);
            if !upper.is_empty() || !lower.is_empty() {
                return (upper, lower);
            }
        }
        return (String::new(), String::new());
    }

    let mut iter = s.splitn(2, char::is_whitespace);
    let upper = iter.next().unwrap_or("").trim().to_string();
    let lower = iter.next().unwrap_or("").trim().to_string();
    (upper, lower)
}

fn xml_escape(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => result.push_str("&amp;"),
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            '"' => result.push_str("&quot;"),
            '\'' => result.push_str("&apos;"),
            other => result.push(other),
        }
    }
    result
}

fn estimate_text_width(text: &str) -> f64 {
    text.chars()
        .map(|c| if c as u32 > 0x2E7F { 100.0 } else { 60.0 })
        .sum()
}

fn linear_gradient(id: &str, x1: f64, y1: f64, x2: f64, y2: f64, stops: &[(f64, &str)]) -> String {
    let mut s = format!(
        r#"<linearGradient id="{}" x1="{}" y1="{}" x2="{}" y2="{}" gradientUnits="userSpaceOnUse">"#,
        id, x1, y1, x2, y2
    );
    for (offset, color) in stops {
        s.push_str(&format!(r#"<stop offset="{}" stop-color="{}"/>"#, offset, color));
    }
    s.push_str("</linearGradient>");
    s
}

fn build_svg(upper: &str, lower: &str) -> String {
    let upper_width = estimate_text_width(upper);
    let lower_width = estimate_text_width(lower);
    let offset_x = DEFAULT_OFFSET_X as f64;

    // Canvas dimensions matching the TypeScript reference
    let canvas_width = (upper_width + 80.0)
        .max(lower_width + offset_x + 90.0)
        .max(300.0) as u32;
    let canvas_height = 270u32;

    let upper_esc = xml_escape(upper);
    let lower_esc = xml_escape(lower);

    let upx = 70.0f64;
    let upy = 100.0f64;

    let offset_y = 130.0f64;
    let lpx = offset_x + 130.0;
    let lpy = offset_y + 100.0;

    let ug1 = linear_gradient("ug1", 0.0, 24.0, 0.0, 122.0, &[
        (0.0,  "rgb(0,15,36)"),
        (0.1,  "rgb(255,255,255)"),
        (0.18, "rgb(55,58,59)"),
        (0.25, "rgb(55,58,59)"),
        (0.5,  "rgb(200,200,200)"),
        (0.75, "rgb(55,58,59)"),
        (0.85, "rgb(25,20,31)"),
        (0.91, "rgb(240,240,240)"),
        (0.95, "rgb(166,175,194)"),
        (1.0,  "rgb(50,50,50)"),
    ]);

    let ug2 = linear_gradient("ug2", 0.0, 20.0, 0.0, 100.0, &[
        (0.0,  "rgb(253,241,0)"),
        (0.25, "rgb(245,253,187)"),
        (0.4,  "rgb(255,255,255)"),
        (0.75, "rgb(253,219,9)"),
        (0.9,  "rgb(127,53,0)"),
        (1.0,  "rgb(243,196,11)"),
    ]);

    let ug3 = linear_gradient("ug3", 0.0, 20.0, 0.0, 100.0, &[
        (0.0,  "rgb(255,100,0)"),
        (0.5,  "rgb(123,0,0)"),
        (0.51, "rgb(240,0,0)"),
        (1.0,  "rgb(5,0,0)"),
    ]);

    let ug4 = linear_gradient("ug4", 0.0, 20.0, 0.0, 100.0, &[
        (0.0,  "rgb(230,0,0)"),
        (0.5,  "rgb(230,0,0)"),
        (0.51, "rgb(240,0,0)"),
        (1.0,  "rgb(5,0,0)"),
    ]);

    let lg1 = linear_gradient("lg1", offset_x, offset_y + 20.0, offset_x, offset_y + 118.0, &[
        (0.0,  "rgb(0,15,36)"),
        (0.25, "rgb(250,250,250)"),
        (0.5,  "rgb(150,150,150)"),
        (0.75, "rgb(55,58,59)"),
        (0.85, "rgb(25,20,31)"),
        (0.91, "rgb(240,240,240)"),
        (0.95, "rgb(166,175,194)"),
        (1.0,  "rgb(50,50,50)"),
    ]);

    let lg2 = linear_gradient("lg2", offset_x, offset_y + 20.0, offset_x, offset_y + 100.0, &[
        (0.0,  "rgb(16,25,58)"),
        (0.03, "rgb(255,255,255)"),
        (0.08, "rgb(16,25,58)"),
        (0.2,  "rgb(16,25,58)"),
        (1.0,  "rgb(16,25,58)"),
    ]);

    let lg3 = linear_gradient("lg3", offset_x, offset_y + 20.0, offset_x, offset_y + 100.0, &[
        (0.0,  "rgb(245,246,248)"),
        (0.15, "rgb(255,255,255)"),
        (0.35, "rgb(195,213,220)"),
        (0.5,  "rgb(160,190,201)"),
        (0.51, "rgb(160,190,201)"),
        (0.52, "rgb(196,215,222)"),
        (1.0,  "rgb(255,255,255)"),
    ]);

    // Upper text font spec
    let uf = format!(
        r#"font-family="{}" font-weight="900" font-size="100""#,
        UPPER_FONT_FAMILY
    );
    // Lower text font spec
    let lf = format!(
        r#"font-family="{}" font-weight="900" font-size="100""#,
        LOWER_FONT_FAMILY
    );

    // Common attribute: stroke-linejoin="round" (matches ctx.lineJoin = "round")
    let round = r#"stroke-linejoin="round""#;

    // Build SVG
    let mut svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}">"#,
        canvas_width, canvas_height
    );

    // Defs
    svg.push_str("<defs>");
    svg.push_str(&ug1);
    svg.push_str(&ug2);
    svg.push_str(&ug3);
    svg.push_str(&ug4);
    svg.push_str(&lg1);
    svg.push_str(&lg2);
    svg.push_str(&lg3);
    svg.push_str("</defs>");

    // White background
    svg.push_str(r#"<rect width="100%" height="100%" fill="white"/>"#);

    // Global skew group: ctx.setTransform(1, 0, -0.4, 1, 0, 0)
    // Canvas 2D matrix(a,b,c,d,e,f) → SVG matrix(a,b,c,d,e,f)
    // = matrix(1, 0, -0.4, 1, 0, 0)
    svg.push_str(r#"<g transform="matrix(1,0,-0.4,1,0,0)">"#);

    // ============================================================
    // UPPER TEXT LAYERS
    // ============================================================

    // Layer 1: Black stroke outline, lineWidth=18, at (upx+4, upy+3)
    svg.push_str(&format!(
        r##"<text x="{}" y="{}" {} stroke="#000" stroke-width="18" fill="none" {}>{}</text>"##,
        upx + 4.0, upy + 3.0, uf, round, upper_esc
    ));

    // Layer 2: Metallic gradient stroke, lineWidth=17, at (upx+4, upy+3)
    svg.push_str(&format!(
        r##"<text x="{}" y="{}" {} stroke="url(#ug1)" stroke-width="17" fill="none" {}>{}</text>"##,
        upx + 4.0, upy + 3.0, uf, round, upper_esc
    ));

    // Layer 3: Black stroke, lineWidth=10, at (upx, upy)
    svg.push_str(&format!(
        r##"<text x="{}" y="{}" {} stroke="#000000" stroke-width="10" fill="none" {}>{}</text>"##,
        upx, upy, uf, round, upper_esc
    ));

    // Layer 4: Gold gradient stroke, lineWidth=8, at (upx, upy)
    svg.push_str(&format!(
        r##"<text x="{}" y="{}" {} stroke="url(#ug2)" stroke-width="8" fill="none" {}>{}</text>"##,
        upx, upy, uf, round, upper_esc
    ));

    // Layer 5: Black thin stroke, lineWidth=4, at (upx+2, upy-2)
    svg.push_str(&format!(
        r##"<text x="{}" y="{}" {} stroke="#000" stroke-width="4" fill="none" {}>{}</text>"##,
        upx + 2.0, upy - 2.0, uf, round, upper_esc
    ));

    // Layer 6: White thin stroke, lineWidth=4, at (upx, upy-2)
    svg.push_str(&format!(
        r##"<text x="{}" y="{}" {} stroke="#FFFFFF" stroke-width="4" fill="none" {}>{}</text>"##,
        upx, upy - 2.0, uf, round, upper_esc
    ));

    // Layer 7: Red gradient fill, at (upx, upy-2)
    svg.push_str(&format!(
        r##"<text x="{}" y="{}" {} fill="url(#ug3)" stroke="none">{}</text>"##,
        upx, upy - 2.0, uf, upper_esc
    ));

    // Layer 8: Red gradient stroke (final), at (upx, upy-2)
    // The TS sets lineWidth=1 before fillText but strokeText uses the default lineWidth;
    // the strokeText here uses whatever lineWidth was last set (1), so stroke-width="1"
    svg.push_str(&format!(
        r##"<text x="{}" y="{}" {} stroke="url(#ug4)" stroke-width="1" fill="none" {}>{}</text>"##,
        upx, upy - 2.0, uf, round, upper_esc
    ));

    // ============================================================
    // LOWER TEXT LAYERS
    // ============================================================

    // Layer 1: Black stroke, lineWidth=17, at (lpx+4, lpy+3)
    svg.push_str(&format!(
        r##"<text x="{}" y="{}" {} stroke="#000" stroke-width="17" fill="none" {}>{}</text>"##,
        lpx + 4.0, lpy + 3.0, lf, round, lower_esc
    ));

    // Layer 2: Metallic gradient stroke, lineWidth=14, at (lpx+4, lpy+3)
    svg.push_str(&format!(
        r##"<text x="{}" y="{}" {} stroke="url(#lg1)" stroke-width="14" fill="none" {}>{}</text>"##,
        lpx + 4.0, lpy + 3.0, lf, round, lower_esc
    ));

    // Layer 3: Dark blue stroke, lineWidth=12, at (lpx, lpy)
    svg.push_str(&format!(
        r##"<text x="{}" y="{}" {} stroke="#10193A" stroke-width="12" fill="none" {}>{}</text>"##,
        lpx, lpy, lf, round, lower_esc
    ));

    // Layer 4: Light gray stroke, lineWidth=7, at (lpx, lpy)
    svg.push_str(&format!(
        r##"<text x="{}" y="{}" {} stroke="#DDDDDD" stroke-width="7" fill="none" {}>{}</text>"##,
        lpx, lpy, lf, round, lower_esc
    ));

    // Layer 5: Dark blue gradient stroke, lineWidth=6, at (lpx, lpy)
    svg.push_str(&format!(
        r##"<text x="{}" y="{}" {} stroke="url(#lg2)" stroke-width="6" fill="none" {}>{}</text>"##,
        lpx, lpy, lf, round, lower_esc
    ));

    // Layer 6: Silver/white gradient fill, at (lpx, lpy-3)
    svg.push_str(&format!(
        r##"<text x="{}" y="{}" {} fill="url(#lg3)" stroke="none">{}</text>"##,
        lpx, lpy - 3.0, lf, lower_esc
    ));

    svg.push_str("</g>");
    svg.push_str("</svg>");

    svg
}

/// Generate the 5k style image as PNG bytes.
pub fn generate_5k_image(upper: &str, lower: &str) -> Vec<u8> {
    use resvg::tiny_skia;
    use resvg::usvg;

    // Load fonts into fontdb
    let mut fontdb = usvg::fontdb::Database::new();
    fontdb.load_font_data(UPPER_FONT_DATA.to_vec());
    fontdb.load_font_data(LOWER_FONT_DATA.to_vec());

    // Build SVG string
    let svg_string = build_svg(upper, lower);

    // Parse SVG with usvg
    let mut opt = usvg::Options::default();
    *opt.fontdb_mut() = fontdb;

    let tree = match usvg::Tree::from_str(&svg_string, &opt) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("SVG parse failed: {}", e);
            return vec![];
        }
    };

    // Render to pixmap
    let size = tree.size().to_int_size();
    let mut pixmap = match tiny_skia::Pixmap::new(size.width(), size.height()) {
        Some(p) => p,
        None => {
            eprintln!("Pixmap creation failed");
            return vec![];
        }
    };

    resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());

    // Encode to PNG
    match pixmap.encode_png() {
        Ok(bytes) => bytes,
        Err(e) => {
            eprintln!("PNG encode failed: {}", e);
            vec![]
        }
    }
}

impl Draw5kFeature {
    pub fn feature_id() -> &'static str {
        "draw5k"
    }

    pub fn feature_name() -> &'static str {
        "5k 图片生成: 5k <上行文字> <下行文字>..."
    }
}

#[async_trait]
impl Feature for Draw5kFeature {
    fn feature_id(&self) -> &str {
        Draw5kFeature::feature_id()
    }

    fn feature_name(&self) -> &str {
        Draw5kFeature::feature_name()
    }

    fn check_command(&self, msg: &Value) -> bool {
        if msg["type"].as_str() != Some("text") {
            return false;
        }
        let text = msg["data"]["text"].as_str().unwrap_or("").trim();
        text.starts_with("5k ") || text.starts_with("-5k ")
    }

    async fn deal_with_message(
        &self,
        _context: &MessageContext,
        msg: &Value,
    ) -> Option<MessageSegment> {
        let text = msg["data"]["text"].as_str().unwrap_or("").trim();

        let args = if let Some(rest) = text.strip_prefix("-5k ") {
            rest
        } else if let Some(rest) = text.strip_prefix("5k ") {
            rest
        } else {
            return None;
        };

        let (upper, lower) = parse_args(args);
        if upper.is_empty() {
            return None;
        }

        // Validate length
        if upper.len() > MAX_LENGTH || lower.len() > MAX_LENGTH {
            log::warn!("Text too long for 5k image");
            return None;
        }

        let png_bytes = generate_5k_image(&upper, &lower);
        if png_bytes.is_empty() {
            log::warn!("Failed to generate 5k image");
            return None;
        }

        let b64 = base64::engine::general_purpose::STANDARD.encode(&png_bytes);
        Some(Segment::image(format!("base64://{}", b64)))
    }
}
