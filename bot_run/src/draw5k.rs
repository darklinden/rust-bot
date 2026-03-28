use crate::feature::{Feature, MessageContext};
use async_trait::async_trait;
use base64::Engine;
use bot_lib::structs::{MessageSegment, Segment};
use image::{Rgba, RgbaImage};
use serde_json::Value;

pub struct Draw5kFeature;

// 5×7 pixel bitmap font covering ASCII 32–126.
// Each character occupies 7 consecutive bytes; each byte encodes one row of 5 pixels
// with the most-significant bit on the left.  The first entry is the space character
// (ASCII 32) and entries are laid out sequentially so that the glyph for character C
// starts at byte ((C as usize - 32) * 7).
static FONT_5X7: &[u8] = &[
    // ' ' (32)
    0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, // '!' (33)
    0b00100, 0b00100, 0b00100, 0b00100, 0b00000, 0b00100, 0b00000, // '"' (34)
    0b01010, 0b01010, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, // '#' (35)
    0b01010, 0b11111, 0b01010, 0b01010, 0b11111, 0b01010, 0b00000, // '$' (36)
    0b00100, 0b01110, 0b10100, 0b01110, 0b00101, 0b11110, 0b00100, // '%' (37)
    0b11000, 0b11001, 0b00010, 0b00100, 0b01000, 0b10011, 0b00011, // '&' (38)
    0b01000, 0b10100, 0b10100, 0b01000, 0b10101, 0b10010, 0b01101, // '\'' (39)
    0b00100, 0b00100, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, // '(' (40)
    0b00010, 0b00100, 0b01000, 0b01000, 0b01000, 0b00100, 0b00010, // ')' (41)
    0b01000, 0b00100, 0b00010, 0b00010, 0b00010, 0b00100, 0b01000, // '*' (42)
    0b00000, 0b00100, 0b10101, 0b01110, 0b10101, 0b00100, 0b00000, // '+' (43)
    0b00000, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000, // ',' (44)
    0b00000, 0b00000, 0b00000, 0b00000, 0b00100, 0b00100, 0b01000, // '-' (45)
    0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000, // '.' (46)
    0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00110, 0b00000, // '/' (47)
    0b00001, 0b00010, 0b00100, 0b00100, 0b01000, 0b10000, 0b00000, // '0' (48)
    0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110, // '1' (49)
    0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110, // '2' (50)
    0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111, // '3' (51)
    0b11111, 0b00010, 0b00100, 0b00010, 0b00001, 0b10001, 0b01110, // '4' (52)
    0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010, // '5' (53)
    0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110, // '6' (54)
    0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110, // '7' (55)
    0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000, // '8' (56)
    0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110, // '9' (57)
    0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100, // ':' (58)
    0b00000, 0b00100, 0b00000, 0b00000, 0b00000, 0b00100, 0b00000, // ';' (59)
    0b00000, 0b00100, 0b00000, 0b00000, 0b00100, 0b00100, 0b01000, // '<' (60)
    0b00010, 0b00100, 0b01000, 0b10000, 0b01000, 0b00100, 0b00010, // '=' (61)
    0b00000, 0b00000, 0b11111, 0b00000, 0b11111, 0b00000, 0b00000, // '>' (62)
    0b10000, 0b01000, 0b00100, 0b00010, 0b00100, 0b01000, 0b10000, // '?' (63)
    0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b00000, 0b00100, // '@' (64)
    0b01110, 0b10001, 0b10111, 0b10101, 0b10111, 0b10000, 0b01111, // 'A' (65)
    0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001, // 'B' (66)
    0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110, // 'C' (67)
    0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110, // 'D' (68)
    0b11100, 0b10010, 0b10001, 0b10001, 0b10001, 0b10010, 0b11100, // 'E' (69)
    0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111, // 'F' (70)
    0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000, // 'G' (71)
    0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111, // 'H' (72)
    0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001, // 'I' (73)
    0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110, // 'J' (74)
    0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100, // 'K' (75)
    0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001, // 'L' (76)
    0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111, // 'M' (77)
    0b10001, 0b11011, 0b10101, 0b10001, 0b10001, 0b10001, 0b10001, // 'N' (78)
    0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001, // 'O' (79)
    0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110, // 'P' (80)
    0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000, // 'Q' (81)
    0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101, // 'R' (82)
    0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001, // 'S' (83)
    0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110, // 'T' (84)
    0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, // 'U' (85)
    0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110, // 'V' (86)
    0b10001, 0b10001, 0b10001, 0b01010, 0b01010, 0b00100, 0b00100, // 'W' (87)
    0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001, // 'X' (88)
    0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b01010, 0b10001, // 'Y' (89)
    0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100, // 'Z' (90)
    0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111, // '[' (91)
    0b01110, 0b01000, 0b01000, 0b01000, 0b01000, 0b01000, 0b01110, // '\\' (92)
    0b10000, 0b01000, 0b00100, 0b00100, 0b00010, 0b00001, 0b00000, // ']' (93)
    0b01110, 0b00010, 0b00010, 0b00010, 0b00010, 0b00010, 0b01110, // '^' (94)
    0b00100, 0b01010, 0b10001, 0b00000, 0b00000, 0b00000, 0b00000, // '_' (95)
    0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b11111, // '`' (96)
    0b01000, 0b00100, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, // 'a' (97)
    0b00000, 0b00000, 0b01110, 0b00001, 0b01111, 0b10001, 0b01111, // 'b' (98)
    0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b10001, 0b11110, // 'c' (99)
    0b00000, 0b00000, 0b01110, 0b10000, 0b10000, 0b10001, 0b01110, // 'd' (100)
    0b00001, 0b00001, 0b01111, 0b10001, 0b10001, 0b10001, 0b01111, // 'e' (101)
    0b00000, 0b00000, 0b01110, 0b10001, 0b11111, 0b10000, 0b01110, // 'f' (102)
    0b00110, 0b01001, 0b01000, 0b11100, 0b01000, 0b01000, 0b01000, // 'g' (103)
    0b00000, 0b01111, 0b10001, 0b10001, 0b01111, 0b00001, 0b01110, // 'h' (104)
    0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b10001, 0b10001, // 'i' (105)
    0b00100, 0b00000, 0b01100, 0b00100, 0b00100, 0b00100, 0b01110, // 'j' (106)
    0b00010, 0b00000, 0b00110, 0b00010, 0b00010, 0b10010, 0b01100, // 'k' (107)
    0b10000, 0b10000, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, // 'l' (108)
    0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110, // 'm' (109)
    0b00000, 0b00000, 0b11010, 0b10101, 0b10101, 0b10001, 0b10001, // 'n' (110)
    0b00000, 0b00000, 0b11110, 0b10001, 0b10001, 0b10001, 0b10001, // 'o' (111)
    0b00000, 0b00000, 0b01110, 0b10001, 0b10001, 0b10001, 0b01110, // 'p' (112)
    0b00000, 0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, // 'q' (113)
    0b00000, 0b01111, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, // 'r' (114)
    0b00000, 0b00000, 0b10110, 0b11001, 0b10000, 0b10000, 0b10000, // 's' (115)
    0b00000, 0b00000, 0b01110, 0b10000, 0b01110, 0b00001, 0b11110, // 't' (116)
    0b01000, 0b01000, 0b11100, 0b01000, 0b01000, 0b01001, 0b00110, // 'u' (117)
    0b00000, 0b00000, 0b10001, 0b10001, 0b10001, 0b10011, 0b01101, // 'v' (118)
    0b00000, 0b00000, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100, // 'w' (119)
    0b00000, 0b00000, 0b10001, 0b10001, 0b10101, 0b10101, 0b01010, // 'x' (120)
    0b00000, 0b00000, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, // 'y' (121)
    0b00000, 0b10001, 0b10001, 0b01111, 0b00001, 0b10001, 0b01110, // 'z' (122)
    0b00000, 0b00000, 0b11111, 0b00010, 0b00100, 0b01000, 0b11111, // '{' (123)
    0b00110, 0b00100, 0b00100, 0b01000, 0b00100, 0b00100, 0b00110, // '|' (124)
    0b00100, 0b00100, 0b00100, 0b00000, 0b00100, 0b00100, 0b00100, // '}' (125)
    0b01100, 0b00100, 0b00100, 0b00010, 0b00100, 0b00100, 0b01100, // '~' (126)
    0b01000, 0b10101, 0b00010, 0b00000, 0b00000, 0b00000, 0b00000,
];

pub const CHAR_W: u32 = 5;
pub const CHAR_H: u32 = 7;
pub const SCALE: u32 = 4;
pub const CELL_W: u32 = CHAR_W * SCALE;
pub const CELL_H: u32 = CHAR_H * SCALE;

// Red-to-yellow gradient (top → bottom): t=0.0 gives pure red, t=1.0 gives red+200g.
// This matches the classic 5k meme colour scheme.
fn gradient_color(t: f32) -> (u8, u8, u8) {
    (255u8, (t * 200.0).clamp(0.0, 255.0) as u8, 0u8)
}

// Non-ASCII / CJK characters cannot be rendered with the 5×7 bitmap, so they are
// treated as full-width and occupy 2 cells.  The extra SCALE accounts for letter-spacing.
pub fn text_pixel_width(text: &str) -> u32 {
    let mut width = 0u32;
    for ch in text.chars() {
        if (ch as u32) < 32 || (ch as u32) > 126 {
            width += CELL_W * 2 + SCALE;
        } else {
            width += CELL_W + SCALE;
        }
    }
    width.saturating_sub(SCALE)
}

fn draw_glyph(img: &mut RgbaImage, ch: char, x: u32, y: u32, img_h: u32) {
    let code = ch as u32;
    if !(32..=126).contains(&code) {
        return;
    }
    let idx = (code - 32) as usize;
    let base = idx * 7;
    if base + 7 > FONT_5X7.len() {
        return;
    }

    for row in 0..CHAR_H {
        let bits = FONT_5X7[base + row as usize];
        for col in 0..CHAR_W {
            if (bits >> (CHAR_W - 1 - col)) & 1 == 0 {
                continue;
            }
            for dy in 0..SCALE {
                for dx in 0..SCALE {
                    let px = x + col * SCALE + dx;
                    let py = y + row * SCALE + dy;
                    if px < img.width() && py < img.height() {
                        let (r, g, b) = gradient_color((py as f32 + 0.5) / img_h as f32);
                        img.put_pixel(px, py, Rgba([r, g, b, 255]));
                    }
                }
            }
        }
    }
}

// CJK characters are rendered as a filled gradient rectangle with a 1-pixel white border,
// giving a rough visual representation of a full-width character.
fn draw_fullwidth_block(img: &mut RgbaImage, x: u32, y: u32, img_h: u32) {
    let block_w = CELL_W * 2;
    let block_h = CELL_H;

    for dy in 0..block_h {
        for dx in 0..block_w {
            if dx == 0 || dy == 0 || dx == block_w - 1 || dy == block_h - 1 {
                continue;
            }
            let px = x + dx;
            let py = y + dy;
            if px < img.width() && py < img.height() {
                let (r, g, b) = gradient_color((py as f32 + 0.5) / img_h as f32);
                img.put_pixel(px, py, Rgba([r, g, b, 255]));
            }
        }
    }
}

fn draw_text_line(img: &mut RgbaImage, text: &str, y: u32) {
    let text_w = text_pixel_width(text);
    let img_w = img.width();
    let img_h = img.height();
    let start_x = if img_w > text_w {
        (img_w - text_w) / 2
    } else {
        4
    };
    let mut cursor_x = start_x;

    for ch in text.chars() {
        let code = ch as u32;
        if !(32..=126).contains(&code) {
            draw_fullwidth_block(img, cursor_x, y, img_h);
            cursor_x += CELL_W * 2 + SCALE;
        } else {
            draw_glyph(img, ch, cursor_x, y, img_h);
            cursor_x += CELL_W + SCALE;
        }
    }
}

pub fn generate_5k_image(upper: &str, lower: &str) -> Vec<u8> {
    let padding_h: u32 = 40;
    let line_gap: u32 = 20;
    let padding_bottom: u32 = 40;
    let padding_lr: u32 = 80;

    let content_w = text_pixel_width(upper).max(text_pixel_width(lower));
    let width = (content_w + padding_lr).max(300);
    let height = (padding_h + CELL_H + line_gap + CELL_H + padding_bottom).max(200);

    let mut img = RgbaImage::new(width, height);
    for pixel in img.pixels_mut() {
        *pixel = Rgba([255, 255, 255, 255]);
    }

    draw_text_line(&mut img, upper, padding_h);
    draw_text_line(&mut img, lower, padding_h + CELL_H + line_gap);

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

// Parses the argument string after the command prefix.
// Quoted form:  `"上文字" "下文字"`  — extracts content between double-quote pairs.
fn trim_both_ends(s: &str) -> String {
    let s = s.trim();
    let mut result = s.to_string();
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

pub fn parse_args(args: &str) -> (String, String) {
    let s = args.trim();

    if s.starts_with('"') || s.starts_with('\'') {
        let quote = if s.starts_with('"') { '"' } else { '\'' };

        let mut close_pos: usize = 0;
        let mut found = false;

        for (i, c) in s[1..].char_indices() {
            if c == quote {
                close_pos = i;
                found = true;
                break;
            }
        }

        if !found {
            return (String::new(), String::new());
        }

        while !s.is_char_boundary(close_pos + 1) {
            close_pos += 1;
        }
        let upper = trim_both_ends(&s[1..close_pos + 1]);
        let lower = trim_both_ends(s[close_pos + 2..].trim());
        if !upper.is_empty() || !lower.is_empty() {
            return (upper, lower);
        }
    }

    let mut iter = s.splitn(2, char::is_whitespace);
    let upper = iter.next().unwrap_or("").trim().to_string();
    let lower = iter.next().unwrap_or("").trim().to_string();
    (upper, lower)
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

        let png_bytes = generate_5k_image(&upper, &lower);
        let b64 = base64::engine::general_purpose::STANDARD.encode(&png_bytes);
        Some(Segment::image(format!("base64://{}", b64)))
    }
}
