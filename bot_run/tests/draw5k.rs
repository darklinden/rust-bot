use bot_run::draw5k::generate_5k_image;
use std::fs;
use std::path::PathBuf;

fn png_header(bytes: &[u8]) -> bool {
    bytes.len() > 8 && bytes[0] == 0x89 && bytes[1] == 0x50 && bytes[2] == 0x4E && bytes[3] == 0x47
}

fn save_png(bytes: &[u8], name: &str) {
    let out_dir = PathBuf::from("test_output");
    fs::create_dir_all(&out_dir).ok();
    let path = out_dir.join(format!("{}.png", name));
    fs::write(&path, bytes).ok();
    println!("Saved: {:?}", path);
}

#[test]
fn generate_hello_world() {
    let png = generate_5k_image("hello", "world");
    assert!(png_header(&png));
    save_png(&png, "hello_world");
}

#[test]
fn generate_chinese_text() {
    let png = generate_5k_image("测试文字", "第二行");
    assert!(png_header(&png));
    save_png(&png, "chinese_text");
}

#[test]
fn generate_single_char() {
    let png = generate_5k_image("A", "B");
    assert!(png_header(&png));
    save_png(&png, "single_char");
}

#[test]
fn generate_long_text() {
    let png = generate_5k_image("这是一段比较长的上行文字", "这也是比较长的下行文字内容");
    assert!(png_header(&png));
    save_png(&png, "long_text");
}
