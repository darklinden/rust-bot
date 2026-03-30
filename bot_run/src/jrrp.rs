use crate::feature::{msg_segment_from_string, Feature, MessageContext};
use async_trait::async_trait;
use bot_lib::structs::MessageSegment;
use chrono::Utc;
use num_bigint::BigUint;
use num_traits::ToPrimitive;
use serde_json::Value;
use sha2::{Digest, Sha256};

pub struct JrrpFeature;

impl JrrpFeature {
    pub fn get_luck_value(&self, user_id: i64) -> i32 {
        let day_timestamp = (Utc::now().timestamp() + (8 * 3600)) / 86400; // 以北京时间为基准

        let mut hasher = Sha256::new();
        hasher.update(user_id.to_string().as_bytes());
        hasher.update(day_timestamp.to_string().as_bytes());
        hasher.update("42".as_bytes());
        let hash_result = hasher.finalize();

        let hash_hex = format!("{:x}", hash_result);
        let big = BigUint::parse_bytes(hash_hex.as_bytes(), 16).unwrap();
        let float_val = big.to_f64().unwrap(); // This will be imprecise for large values
        let luck_value = (float_val % 101.0) as u32;

        luck_value as i32
    }

    pub fn get_luck_comment(&self, luck: i32) -> String {
        const LEVELS: [i32; 5] = [0, 20, 40, 60, 80];
        const JACKPOTS: [i32; 4] = [0, 42, 77, 100];

        if JACKPOTS.contains(&luck) {
            match luck {
                0 => "怎，怎么会这样……".to_string(),
                42 => "感觉可以参透宇宙的真理。".to_string(),
                77 => "要不要去抽一发卡试试呢……？".to_string(),
                100 => "买彩票可能会中大奖哦！".to_string(),
                _ => String::new(),
            }
        } else {
            let key_index = LEVELS.iter().position(|&l| luck <= l);
            let key = match key_index {
                Some(0) => LEVELS[0],
                Some(i) => LEVELS[i - 1],
                None => *LEVELS.last().unwrap(),
            };
            match key {
                0 => "推荐闷头睡大觉。".to_string(),
                20 => "也许今天适合摆烂。".to_string(),
                40 => "又是平凡的一天。".to_string(),
                60 => "太阳当头照，花儿对你笑。".to_string(),
                80 => "出门可能捡到 1 块钱。".to_string(),
                _ => String::new(),
            }
        }
    }
}

#[async_trait]
impl Feature for JrrpFeature {
    fn feature_name(&self) -> &str {
        "今日人品: -jrrp 或 jrrp 查看今日人品"
    }

    fn check_command(&self, msg: &Value) -> bool {
        if msg["type"].as_str() != Some("text") {
            return false;
        }

        let text = msg["data"]["text"]
            .as_str()
            .unwrap_or("")
            .trim()
            .to_lowercase();

        text == "jrrp" || text == "-jrrp"
    }

    async fn deal_with_message(
        &self,
        context: &MessageContext,
        _msg: &Value,
    ) -> Option<MessageSegment> {
        log::info!("Calculating luck for user_id: {}", context.user_id);

        let luck = self.get_luck_value(context.user_id);
        let comment = self.get_luck_comment(luck);
        let name = context.display_name();

        let response = format!("{} 的今日人品是：{}。{}", name, luck, comment);
        Some(msg_segment_from_string(response))
    }
}
