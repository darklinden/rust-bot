use bot_run::feature::MessageContext;
use serde_json::json;

#[test]
fn format_timestamp_positive() {
    use bot_run::dup_check::format_timestamp;

    let result = format_timestamp(8 * 3600);
    assert_eq!(result, "1970/01/01 16:00:00");
}

#[test]
fn format_timestamp_zero() {
    use bot_run::dup_check::format_timestamp;

    let result = format_timestamp(0);
    assert_eq!(result, "1970/01/01 08:00:00");
}

#[test]
fn message_context_display_name_uses_card() {
    let ctx = MessageContext {
        self_id: 1,
        user_id: 2,
        group_id: None,
        message_id: 3,
        message: vec![],
        raw_message: String::new(),
        nickname: "nickname".to_string(),
        card: "card".to_string(),
    };
    assert_eq!(ctx.display_name(), "card");
}

#[test]
fn message_context_display_name_uses_nickname_when_card_empty() {
    let ctx = MessageContext {
        self_id: 1,
        user_id: 2,
        group_id: None,
        message_id: 3,
        message: vec![],
        raw_message: String::new(),
        nickname: "nickname".to_string(),
        card: String::new(),
    };
    assert_eq!(ctx.display_name(), "nickname");
}

#[test]
fn message_context_from_json() {
    let json = json!({
        "self_id": 111,
        "user_id": 222,
        "group_id": 333,
        "message_id": 444,
        "message": ["hello"],
        "raw_message": "raw",
        "sender": {
            "nickname": "nick",
            "card": "card"
        }
    });
    let ctx = MessageContext::from_json(&json);
    assert_eq!(ctx.self_id, 111);
    assert_eq!(ctx.user_id, 222);
    assert_eq!(ctx.group_id, Some(333));
    assert_eq!(ctx.message_id, 444);
    assert_eq!(ctx.nickname, "nick");
    assert_eq!(ctx.card, "card");
    assert_eq!(ctx.raw_message, "raw");
}
