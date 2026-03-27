use crate::feature::{Feature, MessageContext};
use crate::redis_client::redis;
use async_trait::async_trait;
use bot_lib::structs::MessageSegment;
use chrono::{TimeZone, Utc};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::env;
use tokio::time::{sleep, Duration};

// ─── Types ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IAPIRequestResult {
    pub metal: String,
    pub currency: String,
    pub update: String,
    pub prev_close_price: f64,
    pub open_price: f64,
    pub low_price: f64,
    pub high_price: f64,
    pub price: f64,
    pub change_percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ICachedPriceData {
    pub time: i64,
    pub gold_cny: Option<IAPIRequestResult>,
    pub silver_cny: Option<IAPIRequestResult>,
    pub gold_usd: Option<IAPIRequestResult>,
    pub silver_usd: Option<IAPIRequestResult>,
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

pub fn stamp_to_string(timestamp: i64) -> String {
    let dt = Utc.timestamp_opt(timestamp, 0).single();
    match dt {
        Some(dt) => {
            // Shift to UTC+8
            let beijing = dt + chrono::Duration::hours(8);
            beijing.format("%Y-%m-%d %H:%M").to_string()
        }
        None => String::from("N/A"),
    }
}

fn window_ts() -> i64 {
    let now = Utc::now().timestamp();
    (now / 1800) * 1800
}

// ─── Jisu API ─────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct JisuItem {
    #[serde(rename = "type")]
    metal_type: String,
    price: serde_json::Value,
    openingprice: serde_json::Value,
    maxprice: serde_json::Value,
    minprice: serde_json::Value,
    changepercent: serde_json::Value,
    lastclosingprice: serde_json::Value,
    updatetime: String,
}

pub fn parse_f64(v: &serde_json::Value) -> f64 {
    match v {
        Value::Number(n) => n.as_f64().unwrap_or(0.0),
        Value::String(s) => s.parse::<f64>().unwrap_or(0.0),
        _ => 0.0,
    }
}

async fn fetch_jisu_with_retry(
    client: &reqwest::Client,
    url: &str,
    max_retries: u32,
) -> Option<Value> {
    for attempt in 0..=max_retries {
        match client.get(url).send().await {
            Ok(resp) => match resp.json::<Value>().await {
                Ok(json) => {
                    log::debug!(
                        "[Jisu] Successfully fetched data on attempt {}: {}",
                        attempt + 1,
                        json
                    );
                    return Some(json);
                }
                Err(e) => {
                    log::warn!("[Jisu] JSON parse error (attempt {}): {}", attempt + 1, e);
                }
            },
            Err(e) => {
                log::warn!("[Jisu] Request error (attempt {}): {}", attempt + 1, e);
            }
        }
        if attempt < max_retries {
            sleep(Duration::from_millis(500 * (attempt as u64 + 1))).await;
        }
    }
    None
}

async fn fetch_jisu_prices(
    client: &reqwest::Client,
    token: &str,
) -> (Option<IAPIRequestResult>, Option<IAPIRequestResult>) {
    let gold_url = format!("https://api.jisuapi.com/gold/shgold?appkey={}", token);
    let silver_url = format!("https://api.jisuapi.com/gold/shsilver?appkey={}", token);

    let (gold_resp, silver_resp) = tokio::join!(
        fetch_jisu_with_retry(client, &gold_url, 5),
        fetch_jisu_with_retry(client, &silver_url, 5)
    );

    let gold_result = parse_jisu_response(gold_resp, "AU99.99", "XAU", "CNY");
    let silver_result = parse_jisu_response(silver_resp, "Ag99.99", "XAG", "CNY");

    (gold_result, silver_result)
}

fn parse_jisu_response(
    resp: Option<Value>,
    metal_type_filter: &str,
    metal: &str,
    currency: &str,
) -> Option<IAPIRequestResult> {
    let json = resp?;
    let status = json.get("status").and_then(|v| v.as_i64()).unwrap_or(-1);
    if status != 0 {
        log::warn!("[Jisu] Non-zero status: {}", status);
        return None;
    }
    let result = json.get("result")?.as_array()?;
    let item_val = result.iter().find(|item| {
        item.get("type")
            .and_then(|v| v.as_str())
            .map(|t| t == metal_type_filter)
            .unwrap_or(false)
    })?;

    let item: JisuItem = serde_json::from_value(item_val.clone()).ok()?;

    let update_ts = chrono::NaiveDateTime::parse_from_str(&item.updatetime, "%Y-%m-%d %H:%M:%S")
        .map(|dt| dt.and_utc().timestamp() - 8 * 3600)
        .unwrap_or_else(|_| Utc::now().timestamp());

    Some(IAPIRequestResult {
        metal: metal.to_string(),
        currency: currency.to_string(),
        update: stamp_to_string(update_ts),
        prev_close_price: parse_f64(&item.lastclosingprice),
        open_price: parse_f64(&item.openingprice),
        low_price: parse_f64(&item.minprice),
        high_price: parse_f64(&item.maxprice),
        price: parse_f64(&item.price),
        change_percent: parse_f64(&item.changepercent),
    })
}

// ─── GoldAPI ──────────────────────────────────────────────────────────────────

async fn fetch_goldapi_with_retry(
    client: &reqwest::Client,
    url: &str,
    api_key: &str,
    max_retries: u32,
) -> Option<Value> {
    for attempt in 0..=max_retries {
        match client.get(url).header("x-api-key", api_key).send().await {
            Ok(resp) => match resp.json::<Value>().await {
                Ok(json) => {
                    log::debug!(
                        "[GoldAPI] Successfully fetched data on attempt {}: {}",
                        attempt + 1,
                        json
                    );
                    return Some(json);
                }
                Err(e) => {
                    log::warn!(
                        "[GoldAPI] JSON parse error (attempt {}): {}",
                        attempt + 1,
                        e
                    );
                }
            },
            Err(e) => {
                log::warn!("[GoldAPI] Request error (attempt {}): {}", attempt + 1, e);
            }
        }
        if attempt < max_retries {
            sleep(Duration::from_millis(500 * (attempt as u64 + 1))).await;
        }
    }
    None
}

async fn fetch_goldapi_prices(
    client: &reqwest::Client,
    token: &str,
) -> (Option<IAPIRequestResult>, Option<IAPIRequestResult>) {
    let gold_url = "https://api.gold-api.com/price/XAU";
    let silver_url = "https://api.gold-api.com/price/XAG";

    let (gold_resp, silver_resp) = tokio::join!(
        fetch_goldapi_with_retry(client, gold_url, token, 5),
        fetch_goldapi_with_retry(client, silver_url, token, 5)
    );

    let gold_result = parse_goldapi_response(gold_resp, "XAU", "USD");
    let silver_result = parse_goldapi_response(silver_resp, "XAG", "USD");

    (gold_result, silver_result)
}

fn parse_goldapi_response(
    resp: Option<Value>,
    metal: &str,
    currency: &str,
) -> Option<IAPIRequestResult> {
    let json = resp?;

    let price = json
        .get("price")
        .and_then(|v| v.as_f64())
        .or_else(|| {
            json.get("price")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<f64>().ok())
        })
        .unwrap_or(0.0);

    let prev_close = json
        .get("prev_close_price")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let open_price = json
        .get("open_price")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let low_price = json
        .get("low_price")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let high_price = json
        .get("high_price")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    let change_percent = if prev_close > 0.0 {
        (price - prev_close) / prev_close * 100.0
    } else {
        0.0
    };

    let update_ts = json
        .get("timestamp")
        .and_then(|v| v.as_i64())
        .unwrap_or_else(|| Utc::now().timestamp());

    Some(IAPIRequestResult {
        metal: metal.to_string(),
        currency: currency.to_string(),
        update: stamp_to_string(update_ts),
        prev_close_price: prev_close,
        open_price,
        low_price,
        high_price,
        price,
        change_percent,
    })
}

// ─── Currency API ─────────────────────────────────────────────────────────────

async fn fetch_usd_cny_rate(client: &reqwest::Client, token: &str) -> Option<f64> {
    let cache_key = "RATE:USD:CNY";
    let mut conn = match redis().await {
        Ok(c) => c.clone(),
        Err(e) => {
            log::warn!("Redis unavailable: {}", e);
            return None;
        }
    };

    if let Ok(cached) = conn.get::<_, String>(cache_key).await {
        if let Ok(rate) = cached.parse::<f64>() {
            log::debug!("[Currency] Using cached USD/CNY rate: {}", rate);
            return Some(rate);
        }
    }

    let url = format!(
        "https://currencyapi.net/api/v2/rates?base=USD&key={}",
        token
    );

    for attempt in 0u32..=5 {
        match client.get(&url).send().await {
            Ok(resp) => match resp.json::<Value>().await {
                Ok(json) => {
                    let rate = json
                        .get("rates")
                        .and_then(|r| r.get("CNY"))
                        .and_then(|v| v.as_f64())
                        .or_else(|| {
                            json.get("rates")
                                .and_then(|r| r.get("CNY"))
                                .and_then(|v| v.as_str())
                                .and_then(|s| s.parse::<f64>().ok())
                        });

                    if let Some(rate_val) = rate {
                        let _ = conn
                            .set_ex::<_, _, ()>(cache_key, rate_val.to_string(), 7200)
                            .await;
                        log::debug!("[Currency] Fetched USD/CNY rate: {}", rate_val);
                        return Some(rate_val);
                    } else {
                        log::warn!("[Currency] Could not parse rate from response");
                        return None;
                    }
                }
                Err(e) => {
                    log::warn!(
                        "[Currency] JSON parse error (attempt {}): {}",
                        attempt + 1,
                        e
                    );
                }
            },
            Err(e) => {
                log::warn!("[Currency] Request error (attempt {}): {}", attempt + 1, e);
            }
        }
        if attempt < 5 {
            sleep(Duration::from_millis(500 * (attempt as u64 + 1))).await;
        }
    }
    None
}

// ─── Aggregate fetch ─────────────────────────────────────────────────────────

async fn fetch_all_prices() -> ICachedPriceData {
    let jisu_token = env::var("JISU_API_TOKEN").unwrap_or_default();
    let gold_token = env::var("GOLD_API_TOKEN").unwrap_or_default();
    let currency_token = env::var("CURRENCY_RATES_API_TOKEN").unwrap_or_default();

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .unwrap_or_default();

    let (jisu_res, goldapi_res, rate_res) = tokio::join!(
        fetch_jisu_prices(&client, &jisu_token),
        fetch_goldapi_prices(&client, &gold_token),
        fetch_usd_cny_rate(&client, &currency_token)
    );

    let (gold_cny, silver_cny) = jisu_res;
    let (gold_usd_raw, silver_usd_raw) = goldapi_res;

    let gold_usd = gold_usd_raw.map(|mut g| {
        if let Some(rate) = rate_res {
            // price in USD/oz → CNY/g: price * rate / 31.1035
            let cny_per_gram = g.price * rate / 31.1035;
            g.currency = format!("USD|CNY_rate={:.4}|CNY_per_gram={:.4}", rate, cny_per_gram);
        }
        g
    });

    let silver_usd = silver_usd_raw.map(|mut s| {
        if let Some(rate) = rate_res {
            let cny_per_gram = s.price * rate / 31.1035;
            s.currency = format!("USD|CNY_rate={:.4}|CNY_per_gram={:.4}", rate, cny_per_gram);
        }
        s
    });

    ICachedPriceData {
        time: Utc::now().timestamp(),
        gold_cny,
        silver_cny,
        gold_usd,
        silver_usd,
    }
}

// ─── Cache helpers ────────────────────────────────────────────────────────────

async fn get_cached_prices() -> Option<ICachedPriceData> {
    let key = format!("gold:prices:{}", window_ts());
    let mut conn = match redis().await {
        Ok(c) => c.clone(),
        Err(e) => {
            log::warn!("Redis unavailable: {}", e);
            return None;
        }
    };
    let cached: Result<String, _> = conn.get(&key).await;
    if let Ok(json_str) = cached {
        serde_json::from_str::<ICachedPriceData>(&json_str).ok()
    } else {
        None
    }
}

async fn store_cached_prices(data: &ICachedPriceData) {
    let key = format!("gold:prices:{}", window_ts());
    let Ok(conn) = redis().await else {
        log::warn!("Redis unavailable");
        return;
    };
    let mut conn = conn.clone();
    if let Ok(json_str) = serde_json::to_string(data) {
        let _ = conn.set_ex::<_, _, ()>(&key, json_str, 2400).await;
    }
}

async fn get_or_fetch_prices() -> ICachedPriceData {
    if let Some(cached) = get_cached_prices().await {
        log::debug!("[Gold] Using cached price data");
        return cached;
    }
    log::debug!("[Gold] Fetching fresh price data");
    let data = fetch_all_prices().await;
    store_cached_prices(&data).await;
    data
}

// ─── Format helpers ───────────────────────────────────────────────────────────

pub fn format_price(v: f64, decimals: usize) -> String {
    format!("{:.prec$}", v, prec = decimals)
}

pub fn parse_gold_usd_currency(currency_field: &str) -> (f64, f64) {
    // Parse embedded "USD|CNY_rate=X|CNY_per_gram=Y" encoding in currency field
    let mut rate = 0.0f64;
    let mut cny_per_gram = 0.0f64;
    for part in currency_field.split('|') {
        if let Some(val) = part.strip_prefix("CNY_rate=") {
            rate = val.parse::<f64>().unwrap_or(0.0);
        } else if let Some(val) = part.strip_prefix("CNY_per_gram=") {
            cny_per_gram = val.parse::<f64>().unwrap_or(0.0);
        }
    }
    (rate, cny_per_gram)
}

pub fn build_response(data: &ICachedPriceData) -> String {
    let mut lines: Vec<String> = Vec::new();

    lines.push("💰 国内金价 数据来源: Jisu API".to_string());

    if let Some(g) = &data.gold_cny {
        lines.push(format!("黄金价格: {}元/克", format_price(g.price, 2)));
        lines.push(format!(
            "  开盘价: {}元/克 最高价: {}元/克 最低价: {}元/克",
            format_price(g.open_price, 2),
            format_price(g.high_price, 2),
            format_price(g.low_price, 2)
        ));
        lines.push(format!(
            "  涨跌幅: {}% 昨收价: {}元/克 更新时间: {}",
            format_price(g.change_percent, 2),
            format_price(g.prev_close_price, 2),
            g.update
        ));
    } else {
        lines.push("黄金价格: 暂无数据".to_string());
    }

    if let Some(s) = &data.silver_cny {
        lines.push(format!("白银价格: {}元/克", format_price(s.price, 2)));
        lines.push(format!(
            "  开盘价: {}元/克 最高价: {}元/克 最低价: {}元/克",
            format_price(s.open_price, 2),
            format_price(s.high_price, 2),
            format_price(s.low_price, 2)
        ));
        lines.push(format!(
            "  涨跌幅: {}% 昨收价: {}元/克 更新时间: {}",
            format_price(s.change_percent, 2),
            format_price(s.prev_close_price, 2),
            s.update
        ));
    } else {
        lines.push("白银价格: 暂无数据".to_string());
    }

    lines.push("💰 国际金价 数据来源: GoldAPI 汇率数据来源: 汇率 API".to_string());

    if let Some(g) = &data.gold_usd {
        let (rate, cny_per_gram) = parse_gold_usd_currency(&g.currency);
        let usd_line = format!(
            "黄金美元价格: {} USD/盎司{}",
            format_price(g.price, 2),
            if cny_per_gram > 0.0 {
                format!(" 折合 {}元/克", format_price(cny_per_gram, 2))
            } else {
                String::new()
            }
        );
        lines.push(usd_line);
        lines.push(format!(
            "  开盘价: {} USD/盎司 最高价: {} USD/盎司 最低价: {} USD/盎司",
            format_price(g.open_price, 2),
            format_price(g.high_price, 2),
            format_price(g.low_price, 2)
        ));
        lines.push(format!(
            "  涨跌幅: {}% 昨收价: {} USD/盎司 更新时间: {}",
            format_price(g.change_percent, 2),
            format_price(g.prev_close_price, 2),
            g.update
        ));
        if rate > 0.0 {
            lines.push(format!("  USD/CNY 汇率: {}", format_price(rate, 4)));
        }
    } else {
        lines.push("黄金美元价格: 暂无数据".to_string());
    }

    if let Some(s) = &data.silver_usd {
        let (rate, cny_per_gram) = parse_gold_usd_currency(&s.currency);
        let usd_line = format!(
            "白银美元价格: {} USD/盎司{}",
            format_price(s.price, 2),
            if cny_per_gram > 0.0 {
                format!(" 折合 {}元/克", format_price(cny_per_gram, 2))
            } else {
                String::new()
            }
        );
        lines.push(usd_line);
        lines.push(format!(
            "  开盘价: {} USD/盎司 最高价: {} USD/盎司 最低价: {} USD/盎司",
            format_price(s.open_price, 2),
            format_price(s.high_price, 2),
            format_price(s.low_price, 2)
        ));
        lines.push(format!(
            "  涨跌幅: {}% 昨收价: {} USD/盎司 更新时间: {}",
            format_price(s.change_percent, 2),
            format_price(s.prev_close_price, 2),
            s.update
        ));
        if rate > 0.0 {
            lines.push(format!("  USD/CNY 汇率: {}", format_price(rate, 4)));
        }
    } else {
        lines.push("白银美元价格: 暂无数据".to_string());
    }

    lines.join("\n")
}

// ─── Feature impl ─────────────────────────────────────────────────────────────

pub struct GoldFeature;

#[async_trait]
impl Feature for GoldFeature {
    fn feature_name(&self) -> &str {
        "今日金价: -gold 或 gold 查看今日金价"
    }

    fn check_command(&self, msg: &Value) -> bool {
        if msg.get("type").and_then(|v| v.as_str()) != Some("text") {
            return false;
        }
        let text = msg
            .get("data")
            .and_then(|d| d.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .trim()
            .to_lowercase();

        text == "gold" || text == "-gold"
    }

    async fn deal_with_message(
        &self,
        _context: &MessageContext,
        _msg: &Value,
    ) -> Option<MessageSegment> {
        let data = get_or_fetch_prices().await;
        let response = build_response(&data);
        Some(MessageSegment::Text {
            data: bot_lib::structs::TextData { text: response },
        })
    }
}
