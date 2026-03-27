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

// ─── Constants ───────────────────────────────────────────────────────────────

const API_REQUEST_RETRIES: u32 = 5;
const FETCH_INTERVAL_MS: i64 = 30 * 60 * 1000;
const CACHE_EXPIRE: u64 = 40 * 60;
const OUNCE_TO_GRAM: f64 = 31.1035;

// ─── Types ───────────────────────────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IAPIRequestResult {
    pub metal: String,
    pub currency: String,
    pub update: String,
    pub prev_close_price: String,
    pub open_price: String,
    pub low_price: String,
    pub high_price: String,
    pub price: String,
    pub change_percent: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ICachedPriceData {
    pub time: i64,
    pub gold_cny: Option<IAPIRequestResult>,
    pub silver_cny: Option<IAPIRequestResult>,
    pub gold_usd: Option<IAPIRequestResult>,
    pub silver_usd: Option<IAPIRequestResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedRateData {
    time: i64,
    update_time: String,
    usd_cny: Option<f64>,
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

pub fn stamp_to_string(timestamp: i64) -> String {
    let seconds = timestamp / 1000;
    let dt = Utc.timestamp_opt(seconds, 0).single();
    match dt {
        Some(dt) => {
            let beijing = dt + chrono::Duration::hours(8);
            beijing.format("%Y-%m-%d %H:%M").to_string()
        }
        None => String::from("N/A"),
    }
}

fn window_ts() -> i64 {
    let now_ms = Utc::now().timestamp_millis();
    (now_ms / FETCH_INTERVAL_MS) * FETCH_INTERVAL_MS
}

pub fn parse_f64(v: &serde_json::Value) -> f64 {
    match v {
        Value::Number(n) => n.as_f64().unwrap_or(0.0),
        Value::String(s) => s.parse::<f64>().unwrap_or(0.0),
        _ => 0.0,
    }
}

fn value_to_string(v: &serde_json::Value) -> Option<String> {
    match v {
        Value::Number(n) => Some(n.to_string()),
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        _ => None,
    }
}

pub fn format_price(v: f64, decimals: usize) -> String {
    format!("{:.prec$}", v, prec = decimals)
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
            sleep(Duration::from_millis(1000)).await;
        }
    }
    None
}

async fn fetch_jisu_prices(
    client: &reqwest::Client,
    token: &str,
) -> (Option<IAPIRequestResult>, Option<IAPIRequestResult>) {
    let gold_url = format!("https://api.jisuapi.com/gold/shgold?appkey={}", token);
    let silver_url = format!("https://api.jisuapi.com/silver/shgold?appkey={}", token);

    let (gold_resp, silver_resp) = tokio::join!(
        fetch_jisu_with_retry(client, &gold_url, API_REQUEST_RETRIES),
        fetch_jisu_with_retry(client, &silver_url, API_REQUEST_RETRIES)
    );

    let gold_result = parse_jisu_gold_response(gold_resp);
    let silver_result = parse_jisu_silver_response(silver_resp);

    (gold_result, silver_result)
}

fn parse_jisu_gold_response(resp: Option<Value>) -> Option<IAPIRequestResult> {
    let json = resp?;
    let status = json.get("status").and_then(|v| {
        v.as_i64().or_else(|| v.as_str().and_then(|s| s.parse::<i64>().ok()))
    }).unwrap_or(-1);
    if status != 0 {
        log::warn!("[Jisu] Non-zero status for gold: {}", status);
        return None;
    }
    let result = json.get("result")?.as_array()?;
    let item_val = result.iter().find(|item| {
        item.get("type")
            .and_then(|v| v.as_str())
            .map(|t| t == "AU99.99")
            .unwrap_or(false)
    })?;

    let item: JisuItem = serde_json::from_value(item_val.clone()).ok()?;

    Some(IAPIRequestResult {
        metal: "XAU".to_string(),
        currency: "CNY".to_string(),
        update: item.updatetime.clone(),
        prev_close_price: value_to_string(&item.lastclosingprice).unwrap_or_else(|| "N/A".to_string()),
        open_price: value_to_string(&item.openingprice).unwrap_or_else(|| "N/A".to_string()),
        low_price: value_to_string(&item.minprice).unwrap_or_else(|| "N/A".to_string()),
        high_price: value_to_string(&item.maxprice).unwrap_or_else(|| "N/A".to_string()),
        price: value_to_string(&item.price).unwrap_or_else(|| "N/A".to_string()),
        change_percent: value_to_string(&item.changepercent).unwrap_or_else(|| "N/A".to_string()),
    })
}

fn parse_jisu_silver_response(resp: Option<Value>) -> Option<IAPIRequestResult> {
    let json = resp?;
    let status = json.get("status").and_then(|v| {
        v.as_i64().or_else(|| v.as_str().and_then(|s| s.parse::<i64>().ok()))
    }).unwrap_or(-1);
    if status != 0 {
        log::warn!("[Jisu] Non-zero status for silver: {}", status);
        return None;
    }
    let result = json.get("result")?.as_array()?;
    let item_val = result.iter().find(|item| {
        item.get("type")
            .and_then(|v| v.as_str())
            .map(|t| t == "Ag99.99")
            .unwrap_or(false)
    })?;

    let item: JisuItem = serde_json::from_value(item_val.clone()).ok()?;

    // Silver prices from Jisu are in 元/千克, divide by 1000 for 元/克
    let divide_1000 = |v: &serde_json::Value| -> String {
        let raw = parse_f64(v);
        if raw == 0.0 && !matches!(v, Value::Number(_) | Value::String(_)) {
            return "N/A".to_string();
        }
        let has_value = match v {
            Value::String(s) => !s.is_empty(),
            Value::Number(_) => true,
            _ => false,
        };
        if has_value {
            format!("{:.2}", raw / 1000.0)
        } else {
            "N/A".to_string()
        }
    };

    let change_pct = {
        let has_value = match &item.changepercent {
            Value::String(s) => !s.is_empty(),
            Value::Number(_) => true,
            _ => false,
        };
        if has_value {
            let val = parse_f64(&item.changepercent);
            format!("{:.2}%", val)
        } else {
            "N/A".to_string()
        }
    };

    Some(IAPIRequestResult {
        metal: "XAG".to_string(),
        currency: "CNY".to_string(),
        update: item.updatetime.clone(),
        prev_close_price: divide_1000(&item.lastclosingprice),
        open_price: divide_1000(&item.openingprice),
        low_price: divide_1000(&item.minprice),
        high_price: divide_1000(&item.maxprice),
        price: divide_1000(&item.price),
        change_percent: change_pct,
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
            sleep(Duration::from_millis(1000)).await;
        }
    }
    None
}

async fn fetch_goldapi_prices(
    client: &reqwest::Client,
    token: &str,
) -> (Option<IAPIRequestResult>, Option<IAPIRequestResult>) {
    let now = Utc::now().timestamp();
    let start = now - 24 * 3600;

    let gold_price_url = "https://api.gold-api.com/price/XAU".to_string();
    let silver_price_url = "https://api.gold-api.com/price/XAG".to_string();
    let gold_history_url = format!(
        "https://api.gold-api.com/ohlc/XAU?startTimestamp={}&endTimestamp={}",
        start, now
    );
    let silver_history_url = format!(
        "https://api.gold-api.com/ohlc/XAG?startTimestamp={}&endTimestamp={}",
        start, now
    );

    let (gold_price_resp, gold_history_resp, silver_price_resp, silver_history_resp) = tokio::join!(
        fetch_goldapi_with_retry(client, &gold_price_url, token, API_REQUEST_RETRIES),
        fetch_goldapi_with_retry(client, &gold_history_url, token, API_REQUEST_RETRIES),
        fetch_goldapi_with_retry(client, &silver_price_url, token, API_REQUEST_RETRIES),
        fetch_goldapi_with_retry(client, &silver_history_url, token, API_REQUEST_RETRIES)
    );

    let gold_result = build_goldapi_result(gold_price_resp, gold_history_resp, "XAU");
    let silver_result = build_goldapi_result(silver_price_resp, silver_history_resp, "XAG");

    (gold_result, silver_result)
}

fn build_goldapi_result(
    price_resp: Option<Value>,
    history_resp: Option<Value>,
    metal: &str,
) -> Option<IAPIRequestResult> {
    let price_json = price_resp;
    let history_json = history_resp;

    // Current price from /price endpoint
    let price_str = price_json.as_ref().and_then(|json| {
        json.get("price")
            .and_then(|v| v.as_f64())
            .map(|p| p.to_string())
    });

    // updatedAt from /price endpoint
    let update = price_json.as_ref().and_then(|json| {
        json.get("updatedAt")
            .and_then(|v| v.as_str())
            .and_then(|s| {
                chrono::DateTime::parse_from_rfc3339(s)
                    .ok()
                    .map(|dt| stamp_to_string(dt.timestamp_millis()))
            })
    });

    // Extract OHLC from /ohlc endpoint
    let prev_close = history_json.as_ref().and_then(|json| {
        json.get("close").and_then(|v| v.as_f64()).map(|v| v.to_string())
    });
    let open_price = history_json.as_ref().and_then(|json| {
        json.get("open").and_then(|v| v.as_f64()).map(|v| v.to_string())
    });
    let high_price = history_json.as_ref().and_then(|json| {
        json.get("high").and_then(|v| v.as_f64()).map(|v| v.to_string())
    });
    let low_price = history_json.as_ref().and_then(|json| {
        json.get("low").and_then(|v| v.as_f64()).map(|v| v.to_string())
    });

    // change_percent from openCloseChangePercent, formatted as .toFixed(2) + '%'
    let change_percent = history_json.as_ref().and_then(|json| {
        json.get("openCloseChangePercent")
            .and_then(|v| v.as_f64())
            .map(|v| format!("{:.2}%", v))
    });

    if price_json.is_none() && history_json.is_none() {
        return None;
    }

    Some(IAPIRequestResult {
        metal: metal.to_string(),
        currency: "USD".to_string(),
        update: update.unwrap_or_else(|| "N/A".to_string()),
        prev_close_price: prev_close.unwrap_or_else(|| "N/A".to_string()),
        open_price: open_price.unwrap_or_else(|| "N/A".to_string()),
        low_price: low_price.unwrap_or_else(|| "N/A".to_string()),
        high_price: high_price.unwrap_or_else(|| "N/A".to_string()),
        price: price_str.unwrap_or_else(|| "N/A".to_string()),
        change_percent: change_percent.unwrap_or_else(|| "N/A".to_string()),
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

    let now_ms = Utc::now().timestamp_millis();
    if let Ok(cached) = conn.get::<_, String>(cache_key).await {
        if let Ok(rate_data) = serde_json::from_str::<CachedRateData>(&cached) {
            if let Some(rate) = rate_data.usd_cny {
                if (now_ms - rate_data.time) < FETCH_INTERVAL_MS {
                    log::debug!("[Currency] Using cached USD/CNY rate: {}", rate);
                    return Some(rate);
                }
                log::debug!("[Currency] Cached rate is stale, re-fetching");
            }
        } else {
            if let Ok(rate) = cached.parse::<f64>() {
                log::debug!("[Currency] Using legacy cached USD/CNY rate: {}", rate);
                return Some(rate);
            }
        }
    }

    let url = format!(
        "https://currencyapi.net/api/v2/rates?base=USD&output=json&key={}",
        token
    );

    for attempt in 0u32..=API_REQUEST_RETRIES {
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
                        let updated = json
                            .get("updated")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(0);

                        let rate_data = CachedRateData {
                            time: now_ms,
                            update_time: stamp_to_string(updated * 1000),
                            usd_cny: Some(rate_val),
                        };

                        // Store with no expiry (redis SET, not SETEX)
                        if let Ok(json_str) = serde_json::to_string(&rate_data) {
                            let _ = conn.set::<_, _, ()>(cache_key, json_str).await;
                        }
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
        if attempt < API_REQUEST_RETRIES {
            sleep(Duration::from_millis(1000)).await;
        }
    }
    None
}

// ─── Aggregate fetch ─────────────────────────────────────────────────────────

async fn fetch_all_prices() -> ICachedPriceData {
    let jisu_token = env::var("JISU_API_TOKEN").unwrap_or_default();
    let gold_token = env::var("GOLD_API_TOKEN").unwrap_or_default();

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .unwrap_or_default();

    // Fetch prices from Jisu and GoldAPI in parallel
    let (jisu_res, goldapi_res) = tokio::join!(
        fetch_jisu_prices(&client, &jisu_token),
        fetch_goldapi_prices(&client, &gold_token),
    );

    let (gold_cny, silver_cny) = jisu_res;
    let (gold_usd, silver_usd) = goldapi_res;

    ICachedPriceData {
        time: window_ts(),
        gold_cny,
        silver_cny,
        gold_usd,
        silver_usd,
    }
}

// ─── Cache helpers ────────────────────────────────────────────────────────────

fn get_cache_key(window: i64) -> String {
    format!("gold:prices:{}", window)
}

async fn get_cached_prices(window: i64) -> Option<ICachedPriceData> {
    let key = get_cache_key(window);
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

async fn store_cached_prices(window: i64, data: &ICachedPriceData) {
    let key = get_cache_key(window);
    let Ok(conn) = redis().await else {
        log::warn!("Redis unavailable");
        return;
    };
    let mut conn = conn.clone();
    if let Ok(json_str) = serde_json::to_string(data) {
        let _ = conn.set_ex::<_, _, ()>(&key, json_str, CACHE_EXPIRE).await;
    }
}

async fn get_or_fetch_prices() -> (ICachedPriceData, Option<f64>) {
    let now_ms = Utc::now().timestamp_millis();
    let window = (now_ms / FETCH_INTERVAL_MS) * FETCH_INTERVAL_MS;

    let data = if let Some(cached) = get_cached_prices(window).await {
        if cached.time + FETCH_INTERVAL_MS > now_ms {
            log::debug!("[Gold] Cache hit for window {}, using cached prices", window);
            cached
        } else {
            log::debug!("[Gold] Cache stale, fetching fresh price data");
            let data = fetch_all_prices().await;
            store_cached_prices(window, &data).await;
            data
        }
    } else {
        log::debug!("[Gold] Cache miss for window {}, fetching from API...", window);
        let data = fetch_all_prices().await;
        store_cached_prices(window, &data).await;
        data
    };

    let currency_token = env::var("CURRENCY_RATES_API_TOKEN").unwrap_or_default();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .unwrap_or_default();
    let usd_cny_rate = fetch_usd_cny_rate(&client, &currency_token).await;

    (data, usd_cny_rate)
}

// ─── Format / build response ─────────────────────────────────────────────────

pub fn build_response(data: &ICachedPriceData, usd_cny_rate: Option<f64>) -> String {
    let mut parts: Vec<String> = Vec::new();

    // ── Domestic (CNY) ──
    parts.push("💰 国内金价 数据来源: Jisu API ( https://www.jisuapi.com/ )".to_string());

    if let Some(g) = &data.gold_cny {
        parts.push(format!("黄金价格: {}元/克", g.price));
        parts.push(format!(
            "  开盘价: {}元/克 最高价: {}元/克 最低价: {}元/克",
            g.open_price, g.high_price, g.low_price
        ));
        parts.push(format!(
            "  涨跌幅: {}% 昨收价: {}元/克 更新时间: {}",
            g.change_percent, g.prev_close_price, g.update
        ));
    } else {
        parts.push("黄金价格: 暂无数据".to_string());
    }

    parts.push(String::new());

    if let Some(s) = &data.silver_cny {
        parts.push(format!("白银价格: {}元/克", s.price));
        parts.push(format!(
            "  开盘价: {}元/克 最高价: {}元/克 最低价: {}元/克",
            s.open_price, s.high_price, s.low_price
        ));
        parts.push(format!(
            "  涨跌幅: {}% 昨收价: {}元/克 更新时间: {}",
            s.change_percent, s.prev_close_price, s.update
        ));
    } else {
        parts.push("白银价格: 暂无数据".to_string());
    }

    parts.push(String::new());

    // ── International (USD) ──
    parts.push(
        "💰 国际金价 数据来源: GoldAPI ( https://gold-api.com/ ) 汇率数据来源: 汇率 API ( https://currencyapi.net/ )"
            .to_string(),
    );

    if let Some(g) = &data.gold_usd {
        let cny_str = if let Some(rate) = usd_cny_rate {
            if let Ok(price_f) = g.price.parse::<f64>() {
                format!("{:.2}", price_f * rate / OUNCE_TO_GRAM)
            } else {
                "N/A".to_string()
            }
        } else {
            "N/A".to_string()
        };

        parts.push(format!(
            "黄金美元价格: {} USD/盎司 折合 {}元/克",
            g.price, cny_str
        ));
        parts.push(format!(
            "  开盘价: {} USD/盎司 最高价: {} USD/盎司 最低价: {} USD/盎司",
            g.open_price, g.high_price, g.low_price
        ));
        parts.push(format!(
            "  涨跌幅: {}% 昨收价: {} USD/盎司 更新时间: {}",
            g.change_percent, g.prev_close_price, g.update
        ));
    } else {
        parts.push("黄金美元价格: 暂无数据".to_string());
    }

    parts.push(String::new());

    if let Some(s) = &data.silver_usd {
        let cny_str = if let Some(rate) = usd_cny_rate {
            if let Ok(price_f) = s.price.parse::<f64>() {
                format!("{:.2}", price_f * rate / OUNCE_TO_GRAM)
            } else {
                "N/A".to_string()
            }
        } else {
            "N/A".to_string()
        };

        parts.push(format!(
            "白银美元价格: {} USD/盎司 折合 {}元/克",
            s.price, cny_str
        ));
        parts.push(format!(
            "  开盘价: {} USD/盎司 最高价: {} USD/盎司 最低价: {} USD/盎司",
            s.open_price, s.high_price, s.low_price
        ));
        parts.push(format!(
            "  涨跌幅: {}% 昨收价: {} USD/盎司 更新时间: {}",
            s.change_percent, s.prev_close_price, s.update
        ));
    } else {
        parts.push("白银美元价格: 暂无数据".to_string());
    }

    parts.push(String::new());

    parts.join("\n")
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
        let (data, usd_cny_rate) = get_or_fetch_prices().await;
        let response = build_response(&data, usd_cny_rate);
        Some(MessageSegment::Text {
            data: bot_lib::structs::TextData { text: response },
        })
    }
}
