use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MessageSegment {
    #[serde(rename = "text")]
    Text { data: TextData },
    #[serde(rename = "at")]
    At { data: AtData },
    #[serde(rename = "image")]
    Image { data: ImageData },
    #[serde(rename = "face")]
    Face { data: FaceData },
    #[serde(rename = "reply")]
    Reply { data: ReplyData },
    #[serde(rename = "mface")]
    MFace { data: MFaceData },
    #[serde(rename = "file")]
    File { data: FileData },
    #[serde(rename = "video")]
    Video { data: VideoData },
    #[serde(rename = "record")]
    Record { data: RecordData },
    #[serde(rename = "json")]
    Json { data: JsonData },
    #[serde(rename = "markdown")]
    Markdown { data: MarkdownData },
    #[serde(rename = "music")]
    Music { data: MusicData },
    #[serde(rename = "node")]
    Node { data: NodeData },
    #[serde(rename = "forward")]
    Forward { data: ForwardData },
    #[serde(rename = "contact")]
    Contact { data: ContactData },
    #[serde(rename = "dice")]
    Dice { data: DiceData },
    #[serde(rename = "rps")]
    Rps { data: RpsData },
    #[serde(rename = "poke")]
    Poke { data: PokeData },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextData {
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtData {
    pub qq: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageData {
    pub file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_size: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaceData {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chain_count: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplyData {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MFaceData {
    pub emoji_id: String,
    pub emoji_package_id: String,
    pub key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileData {
    pub file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_size: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoData {
    pub file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumb: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_size: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordData {
    pub file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_size: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonData {
    pub data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkdownData {
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MusicData {
    #[serde(rename = "type")]
    pub music_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub singer: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<Vec<MessageSegment>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nickname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub news: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForwardData {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactData {
    #[serde(rename = "type")]
    pub contact_type: String,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiceData {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpsData {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PokeData {
    #[serde(rename = "type")]
    pub poke_type: String,
    pub id: String,
}

pub struct Segment;

impl Segment {
    pub fn text<T: Into<String>>(text: T) -> MessageSegment {
        MessageSegment::Text {
            data: TextData { text: text.into() },
        }
    }

    pub fn at<T: ToString>(qq: T) -> MessageSegment {
        MessageSegment::At {
            data: AtData { qq: qq.to_string() },
        }
    }

    pub fn at_all() -> MessageSegment {
        MessageSegment::At {
            data: AtData {
                qq: "all".to_string(),
            },
        }
    }

    pub fn reply<T: ToString>(id: T) -> MessageSegment {
        MessageSegment::Reply {
            data: ReplyData { id: id.to_string() },
        }
    }

    pub fn face<T: ToString>(id: T) -> MessageSegment {
        MessageSegment::Face {
            data: FaceData {
                id: id.to_string(),
                result_id: None,
                chain_count: None,
            },
        }
    }

    pub fn image<T: Into<String>>(file: T) -> MessageSegment {
        MessageSegment::Image {
            data: ImageData {
                file: file.into(),
                summary: None,
                sub_type: None,
                url: None,
                file_size: None,
            },
        }
    }

    pub fn file<T: Into<String>>(file: T, name: Option<String>) -> MessageSegment {
        MessageSegment::File {
            data: FileData {
                file: file.into(),
                name,
                file_id: None,
                file_size: None,
            },
        }
    }

    pub fn video<T: Into<String>>(file: T) -> MessageSegment {
        MessageSegment::Video {
            data: VideoData {
                file: file.into(),
                name: None,
                thumb: None,
                url: None,
                file_size: None,
            },
        }
    }

    pub fn record<T: Into<String>>(file: T) -> MessageSegment {
        MessageSegment::Record {
            data: RecordData {
                file: file.into(),
                file_size: None,
            },
        }
    }

    pub fn json<T: Into<String>>(data: T) -> MessageSegment {
        MessageSegment::Json {
            data: JsonData { data: data.into() },
        }
    }

    pub fn markdown<T: Into<String>>(content: T) -> MessageSegment {
        MessageSegment::Markdown {
            data: MarkdownData {
                content: content.into(),
            },
        }
    }

    pub fn music<T: Into<String>>(music_type: T, id: T) -> MessageSegment {
        MessageSegment::Music {
            data: MusicData {
                music_type: music_type.into(),
                id: Some(id.into()),
                url: None,
                audio: None,
                title: None,
                image: None,
                singer: None,
            },
        }
    }

    pub fn forward<T: ToString>(message_id: T) -> MessageSegment {
        MessageSegment::Forward {
            data: ForwardData {
                id: message_id.to_string(),
            },
        }
    }

    pub fn dice() -> MessageSegment {
        MessageSegment::Dice { data: DiceData {} }
    }

    pub fn rps() -> MessageSegment {
        MessageSegment::Rps { data: RpsData {} }
    }

    pub fn mface<T: ToString>(emoji_id: T, emoji_package_id: T, key: T) -> MessageSegment {
        MessageSegment::MFace {
            data: MFaceData {
                emoji_id: emoji_id.to_string(),
                emoji_package_id: emoji_package_id.to_string(),
                key: key.to_string(),
                summary: None,
            },
        }
    }

    pub fn poke<T: ToString>(id: T) -> MessageSegment {
        MessageSegment::Poke {
            data: PokeData {
                poke_type: "".to_string(),
                id: id.to_string(),
            },
        }
    }

    pub fn node_with_id<T: ToString>(id: T) -> MessageSegment {
        MessageSegment::Node {
            data: NodeData {
                id: Some(id.to_string()),
                content: None,
                user_id: None,
                nickname: None,
                source: None,
                news: None,
                summary: None,
                prompt: None,
                time: None,
            },
        }
    }

    pub fn node_with_content(content: Vec<MessageSegment>) -> MessageSegment {
        MessageSegment::Node {
            data: NodeData {
                id: None,
                content: Some(content),
                user_id: None,
                nickname: None,
                source: None,
                news: None,
                summary: None,
                prompt: None,
                time: None,
            },
        }
    }

    pub fn contact<T: Into<String>>(contact_type: T, id: T) -> MessageSegment {
        MessageSegment::Contact {
            data: ContactData {
                contact_type: contact_type.into(),
                id: id.into(),
            },
        }
    }
}
