use bot_lib::structs::MessageSegment;

#[test]
fn record_segment_uses_base64_prefix() {
    let segment = bot_run::loli::LoliFeature::build_tts_record_segment(b"hello");

    match segment {
        MessageSegment::Record { data } => {
            assert_eq!(data.file, "base64://aGVsbG8=");
            assert_eq!(data.file_size, None);
        }
        _ => panic!("expected Record segment"),
    }
}

#[test]
fn tts_base_url_defaults_to_local_service() {
    assert_eq!(
        bot_run::loli::LoliFeature::tts_base_url_from_env(None),
        "http://127.0.0.1:8000"
    );
}

#[test]
fn tts_base_url_normalizes_whitespace_and_slash() {
    assert_eq!(
        bot_run::loli::LoliFeature::tts_base_url_from_env(Some(
            " http://127.0.0.1:9000/ ".to_string()
        )),
        "http://127.0.0.1:9000"
    );
}
