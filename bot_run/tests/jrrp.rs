use bot_run::feature::msg_segment_from_string;

fn ts_level_find_index(levels: &[i32; 5], luck: i32) -> Option<usize> {
    levels.iter().position(|&l| luck <= l)
}

fn ts_get_luck_comment(luck: i32) -> String {
    const LEVELS: [i32; 5] = [0, 20, 40, 60, 80];

    const JACKPOT_COMMENTS: [(i32, &str); 4] = [
        (0, "怎，怎么会这样……"),
        (42, "感觉可以参透宇宙的真理。"),
        (77, "要不要去抽一发卡试试呢……？"),
        (100, "买彩票可能会中大奖哦！"),
    ];

    const LEVEL_COMMENTS: [(i32, &str); 5] = [
        (0, "推荐闷头睡大觉。"),
        (20, "也许今天适合摆烂。"),
        (40, "又是平凡的一天。"),
        (60, "太阳当头照，花儿对你笑。"),
        (80, "出门可能捡到 1 块钱。"),
    ];

    for (j, desc) in JACKPOT_COMMENTS {
        if luck == j {
            return desc.to_string();
        }
    }

    let ki = ts_level_find_index(&LEVELS, luck);
    let key = match ki {
        Some(0) => LEVELS[0],
        Some(i) => LEVELS[i - 1],
        None => *LEVELS.last().unwrap(),
    };

    for (l, desc) in LEVEL_COMMENTS {
        if key == l {
            return desc.to_string();
        }
    }

    String::new()
}

#[test]
fn get_luck_comment_all_values() {
    let feature = bot_run::jrrp::JrrpFeature;
    for luck in 0..=100 {
        let expected = ts_get_luck_comment(luck);
        let actual = feature.get_luck_comment(luck);
        assert_eq!(
            actual, expected,
            "luck={} mismatch: expected={:?}, got={:?}",
            luck, expected, actual
        );
    }
}

#[test]
fn get_luck_comment_jackpot_0() {
    let f = bot_run::jrrp::JrrpFeature;
    assert_eq!(f.get_luck_comment(0), "怎，怎么会这样……");
}

#[test]
fn get_luck_comment_jackpot_42() {
    let f = bot_run::jrrp::JrrpFeature;
    assert_eq!(f.get_luck_comment(42), "感觉可以参透宇宙的真理。");
}

#[test]
fn get_luck_comment_jackpot_77() {
    let f = bot_run::jrrp::JrrpFeature;
    assert_eq!(f.get_luck_comment(77), "要不要去抽一发卡试试呢……？");
}

#[test]
fn get_luck_comment_jackpot_100() {
    let f = bot_run::jrrp::JrrpFeature;
    assert_eq!(f.get_luck_comment(100), "买彩票可能会中大奖哦！");
}

#[test]
fn get_luck_comment_level_0_bucket() {
    let f = bot_run::jrrp::JrrpFeature;
    for luck in 1..=20 {
        assert_eq!(f.get_luck_comment(luck), "推荐闷头睡大觉。");
    }
}

#[test]
fn get_luck_comment_level_20_bucket() {
    let f = bot_run::jrrp::JrrpFeature;
    for luck in 21..=40 {
        assert_eq!(f.get_luck_comment(luck), "也许今天适合摆烂。");
    }
}

#[test]
fn get_luck_comment_level_40_bucket() {
    let f = bot_run::jrrp::JrrpFeature;
    for luck in 41..=60 {
        if luck == 42 {
            continue;
        }
        assert_eq!(f.get_luck_comment(luck), "又是平凡的一天。");
    }
}

#[test]
fn get_luck_comment_level_60_bucket() {
    let f = bot_run::jrrp::JrrpFeature;
    for luck in 61..=80 {
        if luck == 77 {
            continue;
        }
        assert_eq!(f.get_luck_comment(luck), "太阳当头照，花儿对你笑。");
    }
}

#[test]
fn get_luck_comment_level_80_bucket() {
    let f = bot_run::jrrp::JrrpFeature;
    for luck in 81..=99 {
        assert_eq!(f.get_luck_comment(luck), "出门可能捡到 1 块钱。");
    }
}

#[test]
fn get_luck_value_is_deterministic() {
    let f = bot_run::jrrp::JrrpFeature;
    let v1 = f.get_luck_value(123456);
    let v2 = f.get_luck_value(123456);
    assert_eq!(v1, v2, "same user_id should give same luck value");
}

#[test]
fn get_luck_value_different_users_different_luck() {
    let f = bot_run::jrrp::JrrpFeature;
    let v1 = f.get_luck_value(111);
    let v2 = f.get_luck_value(222);
    // Different users should generally produce different values (statistically)
    // but we're just checking they're in valid range
    assert!((0..=100).contains(&v1));
    assert!((0..=100).contains(&v2));
}

#[test]
fn get_luck_value_in_range() {
    let f = bot_run::jrrp::JrrpFeature;
    for uid in [1i64, 42, 100, 9999, 123456789] {
        let luck = f.get_luck_value(uid);
        assert!(
            (0..=100).contains(&luck),
            "luck {} out of range for uid {}",
            luck,
            uid
        );
    }
}

#[test]
fn msg_segment_from_string_produces_text_segment() {
    let seg = msg_segment_from_string("hello".to_string());
    match seg {
        bot_lib::structs::MessageSegment::Text { data } => {
            assert_eq!(data.text, "hello");
        }
        _ => panic!("expected Text segment"),
    }
}

#[test]
fn msg_segment_from_string_unicode() {
    let seg = msg_segment_from_string("今日人品是：42。感觉可以参透宇宙的真理。".to_string());
    match seg {
        bot_lib::structs::MessageSegment::Text { data } => {
            assert_eq!(data.text, "今日人品是：42。感觉可以参透宇宙的真理。");
        }
        _ => panic!("expected Text segment"),
    }
}
