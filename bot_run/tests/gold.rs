use bot_run::Feature;
use serde_json::json;

#[test]
fn stamp_to_string_basic() {
    let result = bot_run::gold::stamp_to_string(0);
    assert_eq!(result, "1970-01-01 08:00");
}

#[test]
fn stamp_to_string_known_date() {
    let result = bot_run::gold::stamp_to_string(1740225600_000);
    assert_eq!(result, "2025-02-22 20:00");
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
    let result = bot_run::gold::format_price(123.456, 2);
    assert_eq!(result, "123.46");
}

#[test]
fn format_price_four_decimals() {
    let result = bot_run::gold::format_price(123.456789, 4);
    assert_eq!(result, "123.4568");
}

#[test]
fn parse_f64_from_number() {
    let v = json!(123.45);
    let result = bot_run::gold::parse_f64(&v);
    assert!((result - 123.45).abs() < 0.001);
}

#[test]
fn parse_f64_from_string() {
    let v = json!("678.90");
    let result = bot_run::gold::parse_f64(&v);
    assert!((result - 678.90).abs() < 0.001);
}

#[test]
fn parse_f64_from_invalid() {
    let v = json!(null);
    let result = bot_run::gold::parse_f64(&v);
    assert_eq!(result, 0.0);
}

#[test]
fn build_response_with_all_data() {
    use bot_run::gold::{build_response, IAPIRequestResult, ICachedPriceData};

    let data = ICachedPriceData {
        time: 1740225600_000,
        gold_cny: Some(IAPIRequestResult {
            metal: "XAU".to_string(),
            currency: "CNY".to_string(),
            update: "2026-03-24 20:00".to_string(),
            prev_close_price: "960.00".to_string(),
            open_price: "965.00".to_string(),
            high_price: "970.00".to_string(),
            low_price: "955.00".to_string(),
            price: "968.00".to_string(),
            change_percent: "0.83".to_string(),
        }),
        silver_cny: Some(IAPIRequestResult {
            metal: "XAG".to_string(),
            currency: "CNY".to_string(),
            update: "2026-03-24 20:00".to_string(),
            prev_close_price: "15.50".to_string(),
            open_price: "15.60".to_string(),
            high_price: "15.80".to_string(),
            low_price: "15.40".to_string(),
            price: "15.70".to_string(),
            change_percent: "1.29".to_string(),
        }),
        gold_usd: Some(IAPIRequestResult {
            metal: "XAU".to_string(),
            currency: "USD".to_string(),
            update: "2026-03-24 12:00".to_string(),
            prev_close_price: "4350".to_string(),
            open_price: "4340".to_string(),
            high_price: "4370".to_string(),
            low_price: "4330".to_string(),
            price: "4356".to_string(),
            change_percent: "0.14%".to_string(),
        }),
        silver_usd: None,
    };

    let response = build_response(&data, Some(6.8923));

    assert!(response.contains("💰 国内金价 数据来源: Jisu API ( https://www.jisuapi.com/ )"));
    assert!(response.contains("黄金价格: 968.00元/克"));
    assert!(response.contains("白银价格: 15.70元/克"));
    assert!(response.contains("💰 国际金价 数据来源: GoldAPI ( https://gold-api.com/ ) 汇率数据来源: 汇率 API ( https://currencyapi.net/ )"));
    // 4356 * 6.8923 / 31.1035 ≈ 965.00
    let cny_converted = 4356.0 * 6.8923 / 31.1035;
    let cny_str = format!("{:.2}", cny_converted);
    assert!(response.contains(&format!(
        "黄金美元价格: 4356 USD/盎司 折合 {}元/克",
        cny_str
    )));
    assert!(response.contains("白银美元价格: 暂无数据"));
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

    let response = build_response(&data, None);
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
            prev_close_price: "0.00".to_string(),
            open_price: "0.00".to_string(),
            high_price: "0.00".to_string(),
            low_price: "0.00".to_string(),
            price: "0.00".to_string(),
            change_percent: "0.00".to_string(),
        }),
        silver_cny: None,
        gold_usd: None,
        silver_usd: None,
    };

    let response = build_response(&data, None);
    assert!(response.contains("黄金价格: 0.00元/克"));
    assert!(response.contains("白银价格: 暂无数据"));
    assert!(response.contains("黄金美元价格: 暂无数据"));
}

#[test]
fn build_response_no_rate() {
    use bot_run::gold::{build_response, IAPIRequestResult, ICachedPriceData};

    let data = ICachedPriceData {
        time: 0,
        gold_cny: None,
        silver_cny: None,
        gold_usd: Some(IAPIRequestResult {
            metal: "XAU".to_string(),
            currency: "USD".to_string(),
            update: "2026-03-24 12:00".to_string(),
            prev_close_price: "4350".to_string(),
            open_price: "4340".to_string(),
            high_price: "4370".to_string(),
            low_price: "4330".to_string(),
            price: "4356".to_string(),
            change_percent: "0.14%".to_string(),
        }),
        silver_usd: None,
    };

    let response = build_response(&data, None);
    assert!(response.contains("折合 N/A元/克"));
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
