use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::Value;

static CQ_TAG_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\[CQ:([a-z]+)(?:,([^\]]+))?\]").unwrap());

pub fn cq_decode(s: &str) -> String {
    s.replace("&#44;", ",")
        .replace("&#91;", "[")
        .replace("&#93;", "]")
        .replace("&amp;", "&")
}

pub fn cq_encode(s: &str) -> String {
    s.replace(',', "&#44;")
        .replace('[', "&#91;")
        .replace(']', "&#93;")
        .replace('&', "&amp;")
}

pub fn cq_to_json(msg: &str) -> Vec<Value> {
    let decoded = cq_decode(msg);
    let mut result = Vec::new();
    let mut current_text = String::new();

    let mut chars = decoded.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '[' {
            if !current_text.is_empty() {
                result.push(serde_json::json!({
                    "type": "text",
                    "data": { "text": current_text.clone() }
                }));
                current_text.clear();
            }

            let mut tag_str = String::from("[");
            while let Some(&c) = chars.peek() {
                tag_str.push(c);
                if c == ']' {
                    chars.next();
                    break;
                }
                chars.next();
            }

            if let Some(caps) = CQ_TAG_REGEX.captures(&tag_str) {
                let tag_name = caps.get(1).map(|m| m.as_str()).unwrap_or("text");
                let mut data = serde_json::Map::new();

                if let Some(params) = caps.get(2) {
                    for param in params.as_str().split(',') {
                        if let Some((k, v)) = param.split_once('=') {
                            data.insert(k.to_string(), serde_json::Value::String(v.to_string()));
                        }
                    }
                }

                result.push(serde_json::json!({
                    "type": tag_name,
                    "data": data
                }));
            } else if tag_str.starts_with('[') && tag_str.ends_with(']') {
                result.push(serde_json::json!({
                    "type": "text",
                    "data": { "text": tag_str }
                }));
            }
        } else {
            current_text.push(ch);
        }
    }

    if !current_text.is_empty() {
        result.push(serde_json::json!({
            "type": "text",
            "data": { "text": current_text }
        }));
    }

    result
}

pub fn json_to_cq(segments: &[Value]) -> String {
    let mut result = String::new();

    for segment in segments {
        let seg_type = segment
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("text");

        let data = segment
            .get("data")
            .and_then(|v| v.as_object())
            .map(|m| {
                m.iter()
                    .filter(|(_, v)| !v.is_null() && !v.as_str().is_some_and(|s| s.is_empty()))
                    .map(|(k, v)| {
                        if let Some(s) = v.as_str() {
                            format!("{}={}", k, cq_encode(s))
                        } else {
                            format!("{}={}", k, v)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .unwrap_or_default();

        if seg_type == "text" {
            result.push_str(&data);
        } else if data.is_empty() {
            result.push_str(&format!("[CQ:{}]", seg_type));
        } else {
            result.push_str(&format!("[CQ:{},{}]", seg_type, data));
        }
    }

    result
}

pub mod logger {
    use chrono::Local;
    use log::{LevelFilter, Log, Metadata, Record};
    use std::sync::Mutex;

    static LOGGER: SimpleLogger = SimpleLogger {
        level: Mutex::new(LevelFilter::Info),
    };

    pub fn init() {
        log::set_logger(&LOGGER)
            .map(|()| log::set_max_level(LevelFilter::Debug))
            .ok();
    }

    pub fn set_level(level: LevelFilter) {
        *LOGGER.level.lock().unwrap() = level;
    }

    struct SimpleLogger {
        level: Mutex<LevelFilter>,
    }

    impl Log for SimpleLogger {
        fn enabled(&self, metadata: &Metadata) -> bool {
            metadata.level() <= *self.level.lock().unwrap()
        }

        fn log(&self, record: &Record) {
            if self.enabled(record.metadata()) {
                println!(
                    "[{}] {} - {}",
                    Local::now().format("%Y-%m-%d %H:%M:%S"),
                    record.level(),
                    record.args()
                );
            }
        }

        fn flush(&self) {}
    }
}
