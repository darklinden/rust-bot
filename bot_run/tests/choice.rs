use bot_run::choice::ChoiceFeature;
use bot_run::feature::{Feature, MessageContext};
use serde_json::json;

fn make_context(nickname: &str, card: &str, user_id: i64) -> MessageContext {
    MessageContext {
        self_id: 123456,
        user_id,
        group_id: Some(987654),
        message_id: 111,
        message: vec![],
        raw_message: String::new(),
        nickname: nickname.to_string(),
        card: card.to_string(),
    }
}

fn text_msg(content: &str) -> serde_json::Value {
    json!({ "type": "text", "data": { "text": content } })
}

fn image_msg(url: &str) -> serde_json::Value {
    json!({ "type": "image", "data": { "url": url } })
}

#[test]
fn check_command_accepts_choice_prefix() {
    let f = ChoiceFeature;
    assert!(f.check_command(&text_msg("choice 苹果 香蕉")));
    assert!(f.check_command(&text_msg("choice  a  b  c ")));
}

#[test]
fn check_command_accepts_dash_choice_prefix() {
    let f = ChoiceFeature;
    assert!(f.check_command(&text_msg("-choice 苹果 香蕉")));
}

#[test]
fn check_command_accepts_chinese_prefix() {
    let f = ChoiceFeature;
    assert!(f.check_command(&text_msg("帮我选 苹果 香蕉")));
}

#[test]
fn check_command_rejects_non_text() {
    let f = ChoiceFeature;
    assert!(!f.check_command(&image_msg("http://example.com/img.png")));
}

#[test]
fn check_command_rejects_no_prefix() {
    let f = ChoiceFeature;
    assert!(!f.check_command(&text_msg("随便选 苹果 香蕉")));
}



#[tokio::test]
async fn deal_with_message_returns_error_when_less_than_two_options() {
    let f = ChoiceFeature;
    let ctx = make_context("nick", "card", 12345);

    let result = f.deal_with_message(&ctx, &text_msg("choice A")).await;
    assert!(result.is_some());
    let seg = result.unwrap();
    match seg {
        bot_lib::structs::MessageSegment::Text { data } => {
            assert_eq!(data.text, "请至少提供两个选项哦！");
        }
        _ => panic!("expected Text segment"),
    }
}

#[tokio::test]
async fn deal_with_message_returns_error_with_one_option() {
    let f = ChoiceFeature;
    let ctx = make_context("nick", "card", 12345);

    let result = f.deal_with_message(&ctx, &text_msg("帮我选 一")).await;
    assert!(result.is_some());
    let seg = result.unwrap();
    match seg {
        bot_lib::structs::MessageSegment::Text { data } => {
            assert_eq!(data.text, "请至少提供两个选项哦！");
        }
        _ => panic!("expected Text segment"),
    }
}

#[tokio::test]
async fn deal_with_message_returns_option_from_list() {
    let f = ChoiceFeature;
    let ctx = make_context("张三", "", 12345);

    let result = f.deal_with_message(&ctx, &text_msg("帮我选 苹果 香蕉 樱桃")).await;
    assert!(result.is_some());
    let seg = result.unwrap();
    match seg {
        bot_lib::structs::MessageSegment::Text { data } => {
            assert!(data.text.starts_with("帮 张三(12345) 选择了："));
            let chosen = data.text.strip_prefix("帮 张三(12345) 选择了：").unwrap();
            assert!(["苹果", "香蕉", "樱桃"].contains(&chosen));
        }
        _ => panic!("expected Text segment"),
    }
}

#[tokio::test]
async fn deal_with_message_uses_card_over_nickname() {
    let f = ChoiceFeature;
    let ctx = make_context("nickname", "卡片名", 99999);

    let result = f.deal_with_message(&ctx, &text_msg("choice A B")).await;
    assert!(result.is_some());
    let seg = result.unwrap();
    match seg {
        bot_lib::structs::MessageSegment::Text { data } => {
            assert!(data.text.starts_with("帮 卡片名(99999) 选择了："));
        }
        _ => panic!("expected Text segment"),
    }
}

#[tokio::test]
async fn deal_with_message_trims_whitespace_from_options() {
    let f = ChoiceFeature;
    let ctx = make_context("nick", "card", 1);

    let result = f.deal_with_message(&ctx, &text_msg("choice 苹果 香蕉")).await;
    assert!(result.is_some());
    let seg = result.unwrap();
    match seg {
        bot_lib::structs::MessageSegment::Text { data } => {
            let chosen = data.text.strip_prefix("帮 card(1) 选择了：").unwrap();
            assert!(["苹果", "香蕉"].contains(&chosen));
        }
        _ => panic!("expected Text segment"),
    }
}
