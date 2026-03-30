use crate::feature::{Feature, MessageContext};
use ab_glyph::{FontRef, PxScale};
use async_trait::async_trait;
use base64::Engine;
use bot_lib::structs::{MessageSegment, Segment};
use image::{Rgba, RgbaImage};
use serde_json::Value;

pub struct Draw5kFeature;

// Embedded fonts using include_bytes!
static UPPER_FONT_DATA: &[u8] = include_bytes!("../assets/fonts/SourceHanSerif-Heavy.otf");
static LOWER_FONT_DATA: &[u8] = include_bytes!("../assets/fonts/SourceHanSans-Heavy.otf");

// Config constants
const MAX_LENGTH: usize = 42;
const DEFAULT_OFFSET_X: i32 = 200;

// Trim quotes from both ends
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

// Parse command arguments
fn parse_args(args: &str) -> (String, String) {
    let s = args.trim();
    if s.is_empty() {
        return (String::new(), String::new());
    }

    // Handle quoted first argument
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

    // Handle space-separated arguments
    let mut iter = s.splitn(2, char::is_whitespace);
    let upper = iter.next().unwrap_or("").trim().to_string();
    let lower = iter.next().unwrap_or("").trim().to_string();
    (upper, lower)
}

// Linear interpolation
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

// Interpolate between two RGB colors
fn lerp_color(c1: (u8, u8, u8), c2: (u8, u8, u8), t: f32) -> (u8, u8, u8) {
    (
        lerp(c1.0 as f32, c2.0 as f32, t) as u8,
        lerp(c1.1 as f32, c2.1 as f32, t) as u8,
        lerp(c1.2 as f32, c2.2 as f32, t) as u8,
    )
}

// Create gradient colors for upper text stroke
fn upper_stroke_gradient(t: f32) -> (u8, u8, u8) {
    let stops: Vec<(f32, (u8, u8, u8))> = vec![
        (0.0, (253, 241, 0)),
        (0.25, (245, 253, 187)),
        (0.4, (255, 255, 255)),
        (0.75, (253, 219, 9)),
        (0.9, (127, 53, 0)),
        (1.0, (243, 196, 11)),
    ];
    interpolate_gradient(&stops, t)
}

// Create gradient colors for lower text stroke
fn lower_stroke_gradient(t: f32) -> (u8, u8, u8) {
    let stops: Vec<(f32, (u8, u8, u8))> = vec![
        (0.0, (245, 246, 248)),
        (0.15, (255, 255, 255)),
        (0.35, (195, 213, 220)),
        (0.5, (160, 190, 201)),
        (0.51, (160, 190, 201)),
        (0.52, (196, 215, 222)),
        (1.0, (255, 255, 255)),
    ];
    interpolate_gradient(&stops, t)
}

// Interpolate gradient from stops
fn interpolate_gradient(stops: &[(f32, (u8, u8, u8))], t: f32) -> (u8, u8, u8) {
    if t <= stops[0].0 {
        return stops[0].1;
    }
    if t >= stops[stops.len() - 1].0 {
        return stops[stops.len() - 1].1;
    }

    for i in 0..stops.len() - 1 {
        if t >= stops[i].0 && t <= stops[i + 1].0 {
            let local_t = (t - stops[i].0) / (stops[i + 1].0 - stops[i].0);
            return lerp_color(stops[i].1, stops[i + 1].1, local_t);
        }
    }
    stops[stops.len() - 1].1
}

fn load_embedded_font(data: &'static [u8]) -> Option<FontRef<'static>> {
    FontRef::try_from_slice(data).ok()
}

// Generate the 5k style image
pub fn generate_5k_image(upper: &str, lower: &str) -> Vec<u8> {
    let font_upper = match load_embedded_font(UPPER_FONT_DATA) {
        Some(f) => f,
        None => {
            eprintln!("Failed to load upper font");
            return vec![];
        }
    };

    let font_lower = match load_embedded_font(LOWER_FONT_DATA) {
        Some(f) => f,
        None => {
            eprintln!("Failed to load lower font");
            return vec![];
        }
    };

    let scale = PxScale::from(24.0);

    // Measure text widths using rough approximation
    let char_width_estimate = 24.0 * 0.6;
    let upper_width = upper.len() as f32 * char_width_estimate;
    let lower_width = lower.len() as f32 * char_width_estimate;

    let offset_x = DEFAULT_OFFSET_X;

    let canvas_width =
        ((upper_width as i32 + 80).max(lower_width as i32 + offset_x + 90)).max(300) as u32;
    let canvas_height = 270u32;
    let mut img = RgbaImage::new(canvas_width, canvas_height);
    for pixel in img.pixels_mut() {
        *pixel = Rgba([255, 255, 255, 255]);
    }

    // Draw upper text
    draw_upper_text(&mut img, upper, &font_upper, scale);

    // Draw lower text
    draw_lower_text(&mut img, lower, &font_lower, scale, offset_x);

    // Encode to PNG
    let mut png_bytes: Vec<u8> = Vec::new();
    {
        use image::ImageEncoder;
        image::codecs::png::PngEncoder::new(&mut png_bytes)
            .write_image(
                img.as_raw(),
                img.width(),
                img.height(),
                image::ExtendedColorType::Rgba8,
            )
            .expect("PNG encoding failed");
    }

    png_bytes
}

fn draw_upper_text(img: &mut RgbaImage, text: &str, font: &FontRef, scale: PxScale) {
    let pos_x = 17i32;
    let pos_y = 24i32;

    // Layer 1: Thick black stroke
    for dy in [-12, -6, 0, 6, 12] {
        for dx in [-12, -6, 0, 6, 12] {
            draw_text_at(
                img,
                text,
                font,
                scale,
                pos_x + dx + 2,
                pos_y + dy + 2,
                (0, 0, 0),
            );
        }
    }

    // Layer 2: Gradient stroke
    for dy in [-4, -2, 0, 2, 4] {
        for dx in [-4, -2, 0, 2, 4] {
            let t = (dy + 4) as f32 / 8.0;
            let color = upper_stroke_gradient(t.clamp(0.0, 1.0));
            draw_text_at(img, text, font, scale, pos_x + dx, pos_y + dy, color);
        }
    }

    // Layer 3: Fill
    draw_text_at(img, text, font, scale, pos_x, pos_y - 2, (255, 200, 0));
}

fn draw_lower_text(img: &mut RgbaImage, text: &str, font: &FontRef, scale: PxScale, offset_x: i32) {
    let _offset_y = 130i32;
    let pos_x = offset_x + 33;
    let pos_y = 131;

    // Layer 1: Black stroke
    for dy in [-10, -5, 0, 5, 10] {
        for dx in [-10, -5, 0, 5, 10] {
            draw_text_at(
                img,
                text,
                font,
                scale,
                pos_x + dx + 2,
                pos_y + dy + 2,
                (0, 0, 0),
            );
        }
    }

    // Layer 2: Gradient stroke
    for dy in [-3, -1, 0, 1, 3] {
        for dx in [-3, -1, 0, 1, 3] {
            let t = (dy + 3) as f32 / 6.0;
            let color = lower_stroke_gradient(t.clamp(0.0, 1.0));
            draw_text_at(img, text, font, scale, pos_x + dx, pos_y + dy, color);
        }
    }

    // Layer 3: Fill
    draw_text_at(img, text, font, scale, pos_x, pos_y - 3, (200, 210, 220));
}

fn draw_text_at(
    img: &mut RgbaImage,
    text: &str,
    font: &FontRef,
    scale: PxScale,
    x: i32,
    y: i32,
    color: (u8, u8, u8),
) {
    use ab_glyph::{Font, ScaleFont};

    let scaled_font = font.as_scaled(scale);
    let mut x_pos = x as f32;
    let y_pos = y as f32;

    for c in text.chars() {
        let glyph = scaled_font.scaled_glyph(c);
        let mut positioned_glyph = glyph;
        positioned_glyph.position = ab_glyph::point(x_pos, y_pos);
        x_pos += scaled_font.h_advance(positioned_glyph.id);

        if let Some(outlined) = font.outline_glyph(positioned_glyph) {
            let bounds = outlined.px_bounds();
            eprintln!("  Glyph '{}' bounds: {:?}", c, bounds);
            let img_w = img.width();
            let img_h = img.height();

            outlined.draw(|px, py, coverage| {
                let px = bounds.min.x as i32 + px as i32;
                let py = bounds.min.y as i32 + py as i32;

                if px >= 0 && py >= 0 && (px as u32) < img_w && (py as u32) < img_h {
                    let alpha = (coverage * 255.0) as u8;
                    if alpha > 1 {
                        let px = px as u32;
                        let py = py as u32;
                        img.put_pixel(px, py, Rgba([color.0, color.1, color.2, alpha]));
                    }
                }
            });
        }
    }
}

#[async_trait]
impl Feature for Draw5kFeature {
    fn feature_name(&self) -> &str {
        "5k 图片生成: 5k <上行文字> <下行文字>..."
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
