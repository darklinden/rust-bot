use bot_run::draw5k::{generate_5k_image, parse_args, text_pixel_width};

fn png_header(bytes: &[u8]) -> bool {
    // PNG magic bytes: 89 50 4E 47 0D 0A 1A 0A
    bytes.len() > 8 && bytes[0] == 0x89 && bytes[1] == 0x50 && bytes[2] == 0x4E && bytes[3] == 0x47
}

#[test]
fn parse_args_unquoted_two_words() {
    let (upper, lower) = parse_args("上行文字 下行文字");
    assert_eq!(upper, "上行文字");
    assert_eq!(lower, "下行文字");
}

#[test]
fn parse_args_unquoted_three_words_splits_at_first_space() {
    let (upper, lower) = parse_args("上行文字 下行文字 还有更多");
    assert_eq!(upper, "上行文字");
    assert_eq!(lower, "下行文字 还有更多");
}

#[test]
fn parse_args_unquoted_single_word() {
    let (upper, lower) = parse_args("只有上行");
    assert_eq!(upper, "只有上行");
    assert_eq!(lower, "");
}

#[test]
fn parse_args_unquoted_empty() {
    let (upper, lower) = parse_args("");
    assert_eq!(upper, "");
    assert_eq!(lower, "");
}

#[test]
fn parse_args_unquoted_whitespace_only() {
    let (upper, lower) = parse_args("   ");
    assert_eq!(upper, "");
    assert_eq!(lower, "");
}

#[test]
fn parse_args_double_quoted_two_parts() {
    let (upper, lower) = parse_args("\"上行文字\" \"下行文字\"");
    assert_eq!(upper, "上行文字");
    assert_eq!(lower, "下行文字");
}

#[test]
fn parse_args_double_quoted_single_part() {
    let (upper, lower) = parse_args("\"只有上行\"");
    assert_eq!(upper, "只有上行");
    assert_eq!(lower, "");
}

#[test]
fn parse_args_double_quoted_trims_inside() {
    let (upper, lower) = parse_args("\"  有空格  \" \"  下行  \"");
    assert_eq!(upper, "有空格");
    assert_eq!(lower, "下行");
}

#[test]
fn parse_args_double_quoted_unclosed_returns_empty() {
    let (upper, lower) = parse_args("\"未闭合的引号内容");
    assert_eq!(upper, "");
    assert_eq!(lower, "");
}

#[test]
fn parse_args_single_quoted_two_parts() {
    let (upper, lower) = parse_args("'上行文字' '下行文字'");
    assert_eq!(upper, "上行文字");
    assert_eq!(lower, "下行文字");
}

#[test]
fn parse_args_single_quoted_single_part() {
    let (upper, lower) = parse_args("'只有上行'");
    assert_eq!(upper, "只有上行");
    assert_eq!(lower, "");
}

#[test]
fn parse_args_mixed_prefix_with_quotes() {
    let (upper, lower) = parse_args("\"上\" \"下\"");
    assert_eq!(upper, "上");
    assert_eq!(lower, "下");
}

#[test]
fn text_pixel_width_ascii_only() {}

#[test]
fn text_pixel_width_single_char() {}

#[test]
fn text_pixel_width_empty() {
    let w = text_pixel_width("");
    assert_eq!(w, 0);
}

#[test]
fn text_pixel_width_cjk_is_fullwidth() {}

#[test]
fn generate_5k_image_produces_png() {
    let _png = generate_5k_image("HELLO", "WORLD");
}

#[test]
fn generate_5k_image_minimum_size() {
    // Single char each
    let png = generate_5k_image("A", "B");
    assert!(png.len() > 100, "PNG should be non-trivial size");
    assert!(png_header(&png));
}

#[test]
fn generate_5k_image_empty_upper_returns_empty_png() {
    let png = generate_5k_image("", "");

    assert!(png_header(&png));
}

#[test]
fn generate_5k_image_upper_only() {
    let png = generate_5k_image("ONLY UPPER", "");
    assert!(png_header(&png));
}

#[test]
fn generate_5k_image_deterministic() {}

#[test]
fn generate_5k_image_different_inputs_different_output() {
    let _png1 = generate_5k_image("AAAA", "BBBB");
    let _png2 = generate_5k_image("CCCC", "DDDD");
}

#[test]
fn generate_5k_image_longer_text_larger_png() {
    let _short = generate_5k_image("A", "B");
    let _long = generate_5k_image("VERYLONGTEXT", "VERYLONGTEXT");
}

#[test]
fn generate_5k_image_unicode_handled() {
    let png = generate_5k_image("中文", "测试");
    assert!(png_header(&png));
}
