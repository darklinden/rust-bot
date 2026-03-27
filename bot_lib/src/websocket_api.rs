use crate::event_bus::EventBus;
use crate::structs::MessageSegment;
use crate::websocket_base::{NapcatWebSocketBase, WebSocketError, WebSocketOptions};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendPrivateMsgParams {
    pub user_id: i64,
    pub message: Vec<MessageSegment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_escape: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendGroupMsgParams {
    pub group_id: i64,
    pub message: Vec<MessageSegment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_escape: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SendMsgParams {
    User { user_id: i64, message: Vec<MessageSegment> },
    Group { group_id: i64, message: Vec<MessageSegment> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteMsgParams {
    pub message_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetMsgParams {
    pub message_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetForwardMsgParams {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendLikeParams {
    pub user_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub times: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetGroupKickParams {
    pub group_id: i64,
    pub user_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reject_add_request: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetGroupBanParams {
    pub group_id: i64,
    pub user_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetGroupWholeBanParams {
    pub group_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetGroupAdminParams {
    pub group_id: i64,
    pub user_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetGroupCardParams {
    pub group_id: i64,
    pub user_id: i64,
    pub card: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetGroupNameParams {
    pub group_id: i64,
    pub group_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetGroupLeaveParams {
    pub group_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_dismiss: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetGroupSpecialTitleParams {
    pub group_id: i64,
    pub user_id: i64,
    pub special_title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetFriendAddRequestParams {
    pub flag: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approve: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remark: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetFriendRemarkParams {
    pub user_id: i64,
    pub remark: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetGroupAddRequestParams {
    pub flag: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approve: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetStrangerInfoParams {
    pub user_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_cache: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetGroupInfoParams {
    pub group_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_cache: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetGroupListParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_cache: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetGroupMemberInfoParams {
    pub group_id: i64,
    pub user_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_cache: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetGroupMemberListParams {
    pub group_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_cache: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetCookiesParams {
    pub domain: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetRecordParams {
    pub file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub out_format: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetImageParams {
    pub file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetQQProfileParams {
    pub nickname: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub personal_note: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sex: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteFriendParams {
    pub user_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temp_block: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temp_both_del: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrImageParams {
    pub image: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetGroupSystemMsgParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetEssenceMsgListParams {
    pub group_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetGroupAtAllRemainParams {
    pub group_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetGroupPortraitParams {
    pub file: String,
    pub group_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetEssenceMsgParams {
    pub message_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteEssenceMsgParams {
    pub message_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendGroupNoticeParams {
    pub group_id: i64,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetGroupNoticeParams {
    pub group_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadGroupFileParams {
    pub group_id: i64,
    pub file: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteGroupFileParams {
    pub group_id: i64,
    pub file_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateGroupFileFolderParams {
    pub group_id: i64,
    pub folder_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteGroupFolderParams {
    pub group_id: i64,
    pub folder_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetGroupFileSystemInfoParams {
    pub group_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetGroupRootFilesParams {
    pub group_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_count: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetGroupFilesByFolderParams {
    pub group_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetGroupFileUrlParams {
    pub group_id: i64,
    pub file_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadPrivateFileParams {
    pub user_id: i64,
    pub file: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DownloadFileParams {
    Url { url: String, #[serde(skip_serializing_if = "Option::is_none")] headers: Option<String> },
    Base64 { base64: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandleQuickOperationParams {
    pub context: Value,
    pub operation: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetFriendMsgHistoryParams {
    pub user_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_seq: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetGroupMsgHistoryParams {
    pub group_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_seq: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendGroupForwardMsgParams {
    pub group_id: i64,
    pub message: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendPrivateForwardMsgParams {
    pub user_id: i64,
    pub message: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCollectionParams {
    pub raw_data: String,
    pub brief: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetCollectionListParams {
    pub category: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetSelfLongnickParams {
    pub long_nick: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetRecentContactParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchEmojiLikeParams {
    pub message_id: i64,
    pub emoji_id: String,
    pub emoji_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cookie: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetInputStatusParams {
    pub user_id: String,
    pub event_type: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetGroupInfoExParams {
    pub group_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetGroupDetailInfoParams {
    pub group: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetGroupIgnoreAddRequestParams {
    pub group_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelGroupNoticeParams {
    pub group_id: i64,
    pub notice_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FriendPokeParams {
    pub user_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupPokeParams {
    pub group_id: i64,
    pub user_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NcGetUserStatusParams {
    pub user_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetGroupShutListParams {
    pub group_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveGroupFileParams {
    pub group_id: i64,
    pub file_id: String,
    pub current_parent_directory: String,
    pub target_parent_directory: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransGroupFileParams {
    pub group_id: i64,
    pub file_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenameGroupFileParams {
    pub group_id: i64,
    pub file_id: String,
    pub current_parent_directory: String,
    pub new_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetGroupSignParams {
    pub group_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendPacketParams {
    pub cmd: String,
    pub data: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rsp: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetAiRecordParams {
    pub character: String,
    pub group_id: i64,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetAiCharactersParams {
    pub group_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub char_type: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendGroupAiRecordParams {
    pub character: String,
    pub group_id: i64,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SendPokeParams {
    User { user_id: i64 },
    Group { group_id: i64, user_id: i64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetGroupKickMembersParams {
    pub group_id: String,
    pub user_id: Vec<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reject_add_request: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetGroupRobotAddOptionParams {
    pub group_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub robot_member_switch: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub robot_member_examine: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetGroupAddOptionParams {
    pub group_id: String,
    pub add_type: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_question: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_answer: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetGroupSearchParams {
    pub group_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_code_finger_open: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_finger_open: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetDoubtFriendsAddRequestParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetDoubtFriendsAddRequestParams {
    pub flag: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetGroupRemarkParams {
    pub group_id: String,
    pub remark: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetPrivateFileUrlParams {
    pub file_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClickInlineKeyboardButtonParams {
    pub group_id: i64,
    pub bot_appid: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub button_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub callback_data: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub msg_seq: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SetGroupTodoParams {
    BySeq { group_id: String, message_seq: String },
    ById { group_id: String, message_id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetQunAlbumListParams {
    pub group_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadImageToQunAlbumParams {
    pub group_id: String,
    pub album_id: String,
    pub album_name: String,
    pub file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetGroupAlbumMediaListParams {
    pub group_id: String,
    pub album_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attach_info: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoGroupAlbumCommentParams {
    pub group_id: String,
    pub album_id: String,
    pub lloc: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetGroupAlbumMediaLikeParams {
    pub group_id: String,
    pub album_id: String,
    pub lloc: String,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub set: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelGroupAlbumMediaParams {
    pub group_id: String,
    pub album_id: String,
    pub lloc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetDiyOnlineStatusParams {
    pub face_id: i64,
    pub face_type: i64,
    pub wording: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ArkSharePeerParams {
    Group { group_id: String },
    User { user_id: String, #[serde(skip_serializing_if = "Option::is_none")] phone_number: Option<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArkShareGroupParams {
    pub group_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetOnlineStatusParams {
    pub status: i64,
    pub ext_status: i64,
    pub battery_status: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetQQAvatarParams {
    pub file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetFileParams {
    pub file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForwardFriendSingleMsgParams {
    pub message_id: i64,
    pub user_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForwardGroupSingleMsgParams {
    pub message_id: i64,
    pub group_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslateEn2ZhParams {
    pub words: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetMsgEmojiLikeParams {
    pub message_id: i64,
    pub emoji_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub set: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SendForwardMsgParams {
    User { user_id: i64, message: Vec<Value> },
    Group { group_id: i64, message: Vec<Value> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkPrivateMsgAsReadParams {
    pub user_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkGroupMsgAsReadParams {
    pub group_id: i64,
}

pub struct NapcatWebSocket {
    base: Arc<NapcatWebSocketBase>,
}

impl NapcatWebSocket {
    pub fn new(url: impl Into<String>) -> Self {
        let options = WebSocketOptions::from_url(url);
        let base = NapcatWebSocketBase::new(options);
        Self { base }
    }

    pub fn with_options(options: WebSocketOptions) -> Self {
        let base = NapcatWebSocketBase::new(options);
        Self { base }
    }

    pub async fn run(&self) -> Result<(), WebSocketError> {
        self.base.run().await
    }

    pub fn event_bus(&self) -> &EventBus {
        self.base.event_bus()
    }

    pub async fn on<F>(&self, event: &str, handler: F)
    where
        F: Fn(Value) + Send + Sync + 'static,
    {
        self.base.on(event, handler).await;
    }

    pub async fn once<F>(&self, event: &str, handler: F)
    where
        F: Fn(Value) + Send + Sync + 'static,
    {
        self.base.once(event, handler).await;
    }

    pub async fn off(&self, event: &str) {
        self.base.off(event).await;
    }

    pub fn event_receiver(&self) -> tokio::sync::broadcast::Receiver<Value> {
        self.base.event_receiver()
    }

    pub async fn send_private_msg(&self, params: SendPrivateMsgParams) -> Result<Value, WebSocketError> {
        self.base.send_raw("send_private_msg", serde_json::to_value(params).unwrap()).await
    }

    pub async fn send_group_msg(&self, params: SendGroupMsgParams) -> Result<Value, WebSocketError> {
        self.base.send_raw("send_group_msg", serde_json::to_value(params).unwrap()).await
    }

    pub async fn send_msg(&self, params: SendMsgParams) -> Result<Value, WebSocketError> {
        self.base.send_raw("send_msg", serde_json::to_value(params).unwrap()).await
    }

    pub async fn delete_msg(&self, params: DeleteMsgParams) -> Result<Value, WebSocketError> {
        self.base.send_raw("delete_msg", serde_json::to_value(params).unwrap()).await
    }

    pub async fn get_msg(&self, params: GetMsgParams) -> Result<Value, WebSocketError> {
        self.base.send_raw("get_msg", serde_json::to_value(params).unwrap()).await
    }

    pub async fn get_forward_msg(&self, params: GetForwardMsgParams) -> Result<Value, WebSocketError> {
        self.base.send_raw("get_forward_msg", serde_json::to_value(params).unwrap()).await
    }

    pub async fn send_like(&self, params: SendLikeParams) -> Result<Value, WebSocketError> {
        self.base.send_raw("send_like", serde_json::to_value(params).unwrap()).await
    }

    pub async fn set_group_kick(&self, params: SetGroupKickParams) -> Result<Value, WebSocketError> {
        self.base.send_raw("set_group_kick", serde_json::to_value(params).unwrap()).await
    }

    pub async fn set_group_ban(&self, params: SetGroupBanParams) -> Result<Value, WebSocketError> {
        self.base.send_raw("set_group_ban", serde_json::to_value(params).unwrap()).await
    }

    pub async fn set_group_whole_ban(&self, params: SetGroupWholeBanParams) -> Result<Value, WebSocketError> {
        self.base.send_raw("set_group_whole_ban", serde_json::to_value(params).unwrap()).await
    }

    pub async fn set_group_admin(&self, params: SetGroupAdminParams) -> Result<Value, WebSocketError> {
        self.base.send_raw("set_group_admin", serde_json::to_value(params).unwrap()).await
    }

    pub async fn set_group_card(&self, params: SetGroupCardParams) -> Result<Value, WebSocketError> {
        self.base.send_raw("set_group_card", serde_json::to_value(params).unwrap()).await
    }

    pub async fn set_group_name(&self, params: SetGroupNameParams) -> Result<Value, WebSocketError> {
        self.base.send_raw("set_group_name", serde_json::to_value(params).unwrap()).await
    }

    pub async fn set_group_leave(&self, params: SetGroupLeaveParams) -> Result<Value, WebSocketError> {
        self.base.send_raw("set_group_leave", serde_json::to_value(params).unwrap()).await
    }

    pub async fn set_group_special_title(&self, params: SetGroupSpecialTitleParams) -> Result<Value, WebSocketError> {
        self.base.send_raw("set_group_special_title", serde_json::to_value(params).unwrap()).await
    }

    pub async fn set_friend_add_request(&self, params: SetFriendAddRequestParams) -> Result<Value, WebSocketError> {
        self.base.send_raw("set_friend_add_request", serde_json::to_value(params).unwrap()).await
    }

    pub async fn set_friend_remark(&self, params: SetFriendRemarkParams) -> Result<Value, WebSocketError> {
        self.base.send_raw("set_friend_remark", serde_json::to_value(params).unwrap()).await
    }

    pub async fn set_group_add_request(&self, params: SetGroupAddRequestParams) -> Result<Value, WebSocketError> {
        self.base.send_raw("set_group_add_request", serde_json::to_value(params).unwrap()).await
    }

    pub async fn get_login_info(&self) -> Result<Value, WebSocketError> {
        self.base.send_raw("get_login_info", serde_json::json!({})).await
    }

    pub async fn get_stranger_info(&self, params: GetStrangerInfoParams) -> Result<Value, WebSocketError> {
        self.base.send_raw("get_stranger_info", serde_json::to_value(params).unwrap()).await
    }

    pub async fn get_friend_list(&self) -> Result<Value, WebSocketError> {
        self.base.send_raw("get_friend_list", serde_json::json!({})).await
    }

    pub async fn get_group_info(&self, params: GetGroupInfoParams) -> Result<Value, WebSocketError> {
        self.base.send_raw("get_group_info", serde_json::to_value(params).unwrap()).await
    }

    pub async fn get_group_list(&self, params: Option<GetGroupListParams>) -> Result<Value, WebSocketError> {
        let params = params.unwrap_or(GetGroupListParams { no_cache: None });
        self.base.send_raw("get_group_list", serde_json::to_value(params).unwrap()).await
    }

    pub async fn get_group_member_info(&self, params: GetGroupMemberInfoParams) -> Result<Value, WebSocketError> {
        self.base.send_raw("get_group_member_info", serde_json::to_value(params).unwrap()).await
    }

    pub async fn get_group_member_list(&self, params: GetGroupMemberListParams) -> Result<Value, WebSocketError> {
        self.base.send_raw("get_group_member_list", serde_json::to_value(params).unwrap()).await
    }

    pub async fn get_group_honor_info(&self, group_id: i64, type_: Option<&str>) -> Result<Value, WebSocketError> {
        let params = serde_json::json!({ "group_id": group_id, "type": type_.unwrap_or("all") });
        self.base.send_raw("get_group_honor_info", params).await
    }

    pub async fn get_cookies(&self, params: GetCookiesParams) -> Result<Value, WebSocketError> {
        self.base.send_raw("get_cookies", serde_json::to_value(params).unwrap()).await
    }

    pub async fn get_csrf_token(&self) -> Result<Value, WebSocketError> {
        self.base.send_raw("get_csrf_token", serde_json::json!({})).await
    }

    pub async fn get_credentials(&self) -> Result<Value, WebSocketError> {
        self.base.send_raw("get_credentials", serde_json::json!({})).await
    }

    pub async fn get_record(&self, params: GetRecordParams) -> Result<Value, WebSocketError> {
        self.base.send_raw("get_record", serde_json::to_value(params).unwrap()).await
    }

    pub async fn get_image(&self, params: GetImageParams) -> Result<Value, WebSocketError> {
        self.base.send_raw("get_image", serde_json::to_value(params).unwrap()).await
    }

    pub async fn can_send_image(&self) -> Result<Value, WebSocketError> {
        self.base.send_raw("can_send_image", serde_json::json!({})).await
    }

    pub async fn can_send_record(&self) -> Result<Value, WebSocketError> {
        self.base.send_raw("can_send_record", serde_json::json!({})).await
    }

    pub async fn get_status(&self) -> Result<Value, WebSocketError> {
        self.base.send_raw("get_status", serde_json::json!({})).await
    }

    pub async fn get_version_info(&self) -> Result<Value, WebSocketError> {
        self.base.send_raw("get_version_info", serde_json::json!({})).await
    }

    pub async fn clean_cache(&self) -> Result<Value, WebSocketError> {
        self.base.send_raw("clean_cache", serde_json::json!({})).await
    }

    pub async fn bot_exit(&self) -> Result<Value, WebSocketError> {
        self.base.send_raw("bot_exit", serde_json::json!({})).await
    }

    pub async fn set_qq_profile(&self, params: SetQQProfileParams) -> Result<Value, WebSocketError> {
        self.base.send_raw("set_qq_profile", serde_json::to_value(params).unwrap()).await
    }

    pub async fn delete_friend(&self, params: DeleteFriendParams) -> Result<Value, WebSocketError> {
        self.base.send_raw("delete_friend", serde_json::to_value(params).unwrap()).await
    }

    pub async fn mark_msg_as_read(&self, params: Value) -> Result<Value, WebSocketError> {
        self.base.send_raw("mark_msg_as_read", params).await
    }

    pub async fn send_group_forward_msg(&self, params: SendGroupForwardMsgParams) -> Result<Value, WebSocketError> {
        self.base.send_raw("send_group_forward_msg", serde_json::to_value(params).unwrap()).await
    }

    pub async fn send_private_forward_msg(&self, params: SendPrivateForwardMsgParams) -> Result<Value, WebSocketError> {
        self.base.send_raw("send_private_forward_msg", serde_json::to_value(params).unwrap()).await
    }

    pub async fn get_group_msg_history(&self, params: GetGroupMsgHistoryParams) -> Result<Value, WebSocketError> {
        self.base.send_raw("get_group_msg_history", serde_json::to_value(params).unwrap()).await
    }

    pub async fn ocr_image(&self, params: OcrImageParams) -> Result<Value, WebSocketError> {
        self.base.send_raw("ocr_image", serde_json::to_value(params).unwrap()).await
    }

    pub async fn get_group_system_msg(&self, params: Option<GetGroupSystemMsgParams>) -> Result<Value, WebSocketError> {
        let params = params.unwrap_or(GetGroupSystemMsgParams { count: None });
        self.base.send_raw("get_group_system_msg", serde_json::to_value(params).unwrap()).await
    }

    pub async fn get_essence_msg_list(&self, params: GetEssenceMsgListParams) -> Result<Value, WebSocketError> {
        self.base.send_raw("get_essence_msg_list", serde_json::to_value(params).unwrap()).await
    }

    pub async fn get_group_at_all_remain(&self, params: GetGroupAtAllRemainParams) -> Result<Value, WebSocketError> {
        self.base.send_raw("get_group_at_all_remain", serde_json::to_value(params).unwrap()).await
    }

    pub async fn set_group_portrait(&self, params: SetGroupPortraitParams) -> Result<Value, WebSocketError> {
        self.base.send_raw("set_group_portrait", serde_json::to_value(params).unwrap()).await
    }

    pub async fn set_essence_msg(&self, params: SetEssenceMsgParams) -> Result<Value, WebSocketError> {
        self.base.send_raw("set_essence_msg", serde_json::to_value(params).unwrap()).await
    }

    pub async fn delete_essence_msg(&self, params: DeleteEssenceMsgParams) -> Result<Value, WebSocketError> {
        self.base.send_raw("delete_essence_msg", serde_json::to_value(params).unwrap()).await
    }

    pub async fn send_group_notice(&self, params: SendGroupNoticeParams) -> Result<Value, WebSocketError> {
        self.base.send_raw("_send_group_notice", serde_json::to_value(params).unwrap()).await
    }

    pub async fn get_group_notice(&self, params: GetGroupNoticeParams) -> Result<Value, WebSocketError> {
        self.base.send_raw("_get_group_notice", serde_json::to_value(params).unwrap()).await
    }

    pub async fn upload_group_file(&self, params: UploadGroupFileParams) -> Result<Value, WebSocketError> {
        self.base.send_raw("upload_group_file", serde_json::to_value(params).unwrap()).await
    }

    pub async fn delete_group_file(&self, params: DeleteGroupFileParams) -> Result<Value, WebSocketError> {
        self.base.send_raw("delete_group_file", serde_json::to_value(params).unwrap()).await
    }

    pub async fn create_group_file_folder(&self, params: CreateGroupFileFolderParams) -> Result<Value, WebSocketError> {
        self.base.send_raw("create_group_file_folder", serde_json::to_value(params).unwrap()).await
    }

    pub async fn delete_group_folder(&self, params: DeleteGroupFolderParams) -> Result<Value, WebSocketError> {
        self.base.send_raw("delete_group_folder", serde_json::to_value(params).unwrap()).await
    }

    pub async fn get_group_file_system_info(&self, params: GetGroupFileSystemInfoParams) -> Result<Value, WebSocketError> {
        self.base.send_raw("get_group_file_system_info", serde_json::to_value(params).unwrap()).await
    }

    pub async fn get_group_root_files(&self, params: GetGroupRootFilesParams) -> Result<Value, WebSocketError> {
        self.base.send_raw("get_group_root_files", serde_json::to_value(params).unwrap()).await
    }

    pub async fn get_group_files_by_folder(&self, params: GetGroupFilesByFolderParams) -> Result<Value, WebSocketError> {
        self.base.send_raw("get_group_files_by_folder", serde_json::to_value(params).unwrap()).await
    }

    pub async fn get_group_file_url(&self, params: GetGroupFileUrlParams) -> Result<Value, WebSocketError> {
        self.base.send_raw("get_group_file_url", serde_json::to_value(params).unwrap()).await
    }

    pub async fn upload_private_file(&self, params: UploadPrivateFileParams) -> Result<Value, WebSocketError> {
        self.base.send_raw("upload_private_file", serde_json::to_value(params).unwrap()).await
    }

    pub async fn download_file(&self, params: DownloadFileParams) -> Result<Value, WebSocketError> {
        self.base.send_raw("download_file", serde_json::to_value(params).unwrap()).await
    }

    pub async fn handle_quick_operation(&self, params: HandleQuickOperationParams) -> Result<Value, WebSocketError> {
        self.base.send_raw(".handle_quick_operation", serde_json::to_value(params).unwrap()).await
    }
}
