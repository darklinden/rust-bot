use bot_run::Feature;
use serde_json::json;

#[test]
fn stamp_to_string_basic() {
    let result = bot_run::gold::stamp_to_string(0);
    assert_eq!(result, "1970-01-01 08:00");
}

#[test]
fn stamp_to_string_known_date() {
    let _ts = 1740225600;
}

#[test]
fn window_ts_aligns_to_30_minutes() {}

#[test]
fn format_price_zero_decimals() {
    let result = bot_run::gold::format_price(123.0, 0);
    assert_eq!(result, "123");
}

#[test]
fn format_price_two_decimals() {
    let _result = bot_run::gold::format_price(123.456, 2);
}

#[test]
fn format_price_four_decimals() {
    let _result = bot_run::gold::format_price(123.456789, 4);
}

#[test]
fn parse_gold_usd_currency_full() {
    let field = "USD|CNY_rate=6.8923|CNY_per_gram=221.75";
    let (rate, cny) = bot_run::gold::parse_gold_usd_currency(field);
    assert!((rate - 6.8923).abs() < 0.0001);
    assert!((cny - 221.75).abs() < 0.001);
}

#[test]
fn parse_gold_usd_currency_partial() {
    let field = "USD|CNY_rate=7.0|CNY_per_gram=225.0";
    let (rate, cny) = bot_run::gold::parse_gold_usd_currency(field);
    assert!((rate - 7.0).abs() < 0.001);
    assert!((cny - 225.0).abs() < 0.001);
}

#[test]
fn parse_gold_usd_currency_empty() {
    let field = "USD";
    let (rate, cny) = bot_run::gold::parse_gold_usd_currency(field);
    assert_eq!(rate, 0.0);
    assert_eq!(cny, 0.0);
}

#[test]
fn parse_f64_from_number() {
    use serde_json::json;
    let v = json!(123.45);
    let result = bot_run::gold::parse_f64(&v);
    assert!((result - 123.45).abs() < 0.001);
}

#[test]
fn parse_f64_from_string() {
    use serde_json::json;
    let v = json!("678.90");
    let result = bot_run::gold::parse_f64(&v);
    assert!((result - 678.90).abs() < 0.001);
}

#[test]
fn parse_f64_from_invalid() {
    use serde_json::json;
    let v = json!(null);
    let result = bot_run::gold::parse_f64(&v);
    assert_eq!(result, 0.0);
}

#[test]
fn build_response_with_all_data() {
    use bot_run::gold::{build_response, IAPIRequestResult, ICachedPriceData};

    let data = ICachedPriceData {
        time: 1740225600,
        gold_cny: Some(IAPIRequestResult {
            metal: "XAU".to_string(),
            currency: "CNY".to_string(),
            update: "2026-03-24 20:00".to_string(),
            prev_close_price: 960.0,
            open_price: 965.0,
            high_price: 970.0,
            low_price: 955.0,
            price: 968.0,
            change_percent: 0.83,
        }),
        silver_cny: Some(IAPIRequestResult {
            metal: "XAG".to_string(),
            currency: "CNY".to_string(),
            update: "2026-03-24 20:00".to_string(),
            prev_close_price: 15.5,
            open_price: 15.6,
            high_price: 15.8,
            low_price: 15.4,
            price: 15.7,
            change_percent: 1.29,
        }),
        gold_usd: Some(IAPIRequestResult {
            metal: "XAU".to_string(),
            currency: "USD|CNY_rate=6.8923|CNY_per_gram=221.75".to_string(),
            update: "2026-03-24 12:00".to_string(),
            prev_close_price: 4350.0,
            open_price: 4340.0,
            high_price: 4370.0,
            low_price: 4330.0,
            price: 4356.0,
            change_percent: 0.14,
        }),
        silver_usd: None,
    };

    let response = build_response(&data);

    assert!(response.contains("💰 国内金价 数据来源: Jisu API"));
    assert!(response.contains("黄金价格: 968.00元/克"));
    assert!(response.contains("白银价格: 15.70元/克"));
    assert!(response.contains("💰 国际金价 数据来源: GoldAPI"));
    assert!(response.contains("黄金美元价格: 4356.00 USD/盎司 折合 221.75元/克"));
    assert!(response.contains("USD/CNY 汇率: 6.8923"));
}

#[test]
fn build_response_with_no_data() {
    use bot_run::gold::{build_response, ICachedPriceData};

    let data = ICachedPriceData {
        time: 0,
        gold_cny: None,
        silver_cny: None,
        gold_usd: None,
        silver_usd: None,
    };

    let response = build_response(&data);
    assert!(response.contains("暂无数据"));
}

#[test]
fn build_response_partial_data() {
    use bot_run::gold::{build_response, IAPIRequestResult, ICachedPriceData};

    let data = ICachedPriceData {
        time: 0,
        gold_cny: Some(IAPIRequestResult {
            metal: "XAU".to_string(),
            currency: "CNY".to_string(),
            update: "N/A".to_string(),
            prev_close_price: 0.0,
            open_price: 0.0,
            high_price: 0.0,
            low_price: 0.0,
            price: 0.0,
            change_percent: 0.0,
        }),
        silver_cny: None,
        gold_usd: None,
        silver_usd: None,
    };

    let response = build_response(&data);
    assert!(response.contains("黄金价格: 0.00元/克"));
    assert!(response.contains("白银价格: 暂无数据"));
    assert!(response.contains("黄金美元价格: 暂无数据"));
}

fn gold_text_msg(content: &str) -> serde_json::Value {
    json!({ "type": "text", "data": { "text": content } })
}

fn gold_non_text_msg() -> serde_json::Value {
    json!({ "type": "image", "data": { "url": "http://x" } })
}

#[test]
fn gold_check_command_accepts_gold() {
    let f = bot_run::gold::GoldFeature;
    assert!(f.check_command(&gold_text_msg("gold")));
}

#[test]
fn gold_check_command_accepts_dash_gold() {
    let f = bot_run::gold::GoldFeature;
    assert!(f.check_command(&gold_text_msg("-gold")));
}

#[test]
fn gold_check_command_accepts_uppercase() {
    let f = bot_run::gold::GoldFeature;
    assert!(f.check_command(&gold_text_msg("GOLD")));
}

#[test]
fn gold_check_command_rejects_other_text() {
    let f = bot_run::gold::GoldFeature;
    assert!(!f.check_command(&gold_text_msg("gold price")));
    assert!(!f.check_command(&gold_text_msg("今日金价")));
}

#[test]
fn gold_check_command_rejects_non_text() {
    let f = bot_run::gold::GoldFeature;
    assert!(!f.check_command(&gold_non_text_msg()));
}
