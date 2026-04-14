pub mod choice;
pub mod cron;
pub mod draw5k;
pub mod dup_check;
pub mod feature;
pub mod gold;
pub mod image_matting;
pub mod jrrp;
pub mod loli;
pub mod redis_client;
pub mod sdimage;
pub mod video_prompt;

pub use bot_lib::structs::MessageSegment;

pub use self::choice::ChoiceFeature;
pub use self::cron::{CronFeature, CronResult};
pub use self::draw5k::{generate_5k_image};
pub use self::feature::{msg_segment_from_string, Feature, FeatureConfig, FeatureManager, FEATURE_MANAGER, MessageContext};
pub use self::gold::{
    build_gold_svg, build_response, format_price, parse_f64, render_gold_svg_to_png,
    stamp_to_string, IAPIRequestResult, ICachedPriceData,
};
pub use self::image_matting::{ImageMattingFeature, ImageMattingResult, MsgQueue};
pub use self::jrrp::JrrpFeature;
pub use self::sdimage::{build_workflow, percent_encode, resolve_model, SdParams};
pub use self::video_prompt::VideoPromptFeature;
