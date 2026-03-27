use crate::structs::MessageSegment;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum WebSocketUrl {
    BaseUrl {
        base_url: String,
        #[serde(default)]
        access_token: Option<String>,
        #[serde(default)]
        throw_promise: Option<bool>,
        #[serde(default)]
        reconnection: Option<ReconnectionConfig>,
    },
    HostPort {
        protocol: String,
        host: String,
        port: u16,
        #[serde(default)]
        access_token: Option<String>,
        #[serde(default)]
        throw_promise: Option<bool>,
        #[serde(default)]
        reconnection: Option<ReconnectionConfig>,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct ReconnectionConfig {
    pub enable: bool,
    pub attempts: u32,
    pub delay: u64,
    #[serde(skip)]
    pub now_attempts: u32,
}

impl Default for ReconnectionConfig {
    fn default() -> Self {
        Self {
            enable: true,
            attempts: 10,
            delay: 5000,
            now_attempts: 1,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WSConnecting {
    pub reconnection: ReconnectionConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WSOpenRes {
    pub reconnection: ReconnectionConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WSCloseRes {
    pub code: u16,
    pub reason: String,
    pub reconnection: ReconnectionConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum WSErrorRes {
    ResponseError {
        reconnection: ReconnectionConfig,
        error_type: String,
        info: ResponseErrorInfo,
    },
    ConnectError {
        reconnection: ReconnectionConfig,
        error_type: String,
        errors: Vec<Option<ConnectErrorDetail>>,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ResponseErrorInfo {
    pub errno: i64,
    pub message: String,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConnectErrorDetail {
    pub errno: i64,
    pub code: String,
    pub syscall: String,
    pub address: String,
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct APIRequest<T> {
    pub action: String,
    pub params: T,
    pub echo: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct APISuccessResponse<T> {
    pub status: String,
    pub retcode: i64,
    pub data: T,
    pub message: String,
    pub wording: String,
    pub echo: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct APIErrorResponse {
    pub status: String,
    pub retcode: i64,
    pub data: Option<()>,
    pub message: String,
    pub wording: String,
    pub echo: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HeartBeat {
    pub time: i64,
    pub self_id: i64,
    pub post_type: String,
    pub meta_event_type: String,
    pub status: HeartBeatStatus,
    pub interval: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HeartBeatStatus {
    pub online: Option<bool>,
    pub good: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LifeCycle {
    pub time: i64,
    pub self_id: i64,
    pub post_type: String,
    pub meta_event_type: String,
    pub sub_type: String,
}

pub type LifeCycleEnable = LifeCycle;
pub type LifeCycleDisable = LifeCycle;
pub type LifeCycleConnect = LifeCycle;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Message {
    Private(PrivateMessage),
    Group(GroupMessage),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PrivateMessage {
    pub self_id: i64,
    pub user_id: i64,
    pub time: i64,
    pub message_id: i64,
    pub message_seq: i64,
    pub real_id: i64,
    pub message_type: String,
    pub sender: Sender,
    pub raw_message: String,
    pub font: i64,
    pub sub_type: String,
    pub post_type: String,
    pub message_format: String,
    pub message: Vec<serde_json::Value>,
    #[serde(skip)]
    pub quick_action: Option<fn(Vec<MessageSegment>)>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GroupMessage {
    pub self_id: i64,
    pub user_id: i64,
    pub time: i64,
    pub message_id: i64,
    pub message_seq: i64,
    pub real_id: i64,
    pub message_type: String,
    pub sender: GroupSender,
    pub raw_message: String,
    pub font: i64,
    pub sub_type: String,
    pub post_type: String,
    pub message_format: String,
    pub message: Vec<serde_json::Value>,
    pub group_id: i64,
    #[serde(skip)]
    pub quick_action: Option<fn(Vec<MessageSegment>, Option<bool>)>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Sender {
    pub user_id: i64,
    pub nickname: String,
    pub card: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GroupSender {
    pub user_id: i64,
    pub nickname: String,
    pub card: String,
    pub role: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum MessageSent {
    Private(MessageSentPrivate),
    Group(MessageSentGroup),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MessageSentPrivate {
    pub self_id: i64,
    pub user_id: i64,
    pub time: i64,
    pub message_id: i64,
    pub message_seq: i64,
    pub real_id: i64,
    pub message_type: String,
    pub sender: Sender,
    pub raw_message: String,
    pub font: i64,
    pub sub_type: String,
    pub post_type: String,
    pub message_format: String,
    pub message: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MessageSentGroup {
    pub self_id: i64,
    pub user_id: i64,
    pub time: i64,
    pub message_id: i64,
    pub message_seq: i64,
    pub real_id: i64,
    pub message_type: String,
    pub sender: GroupSender,
    pub raw_message: String,
    pub font: i64,
    pub sub_type: String,
    pub post_type: String,
    pub message_format: String,
    pub message: Vec<serde_json::Value>,
    pub group_id: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Request {
    Friend(RequestFriend),
    Group(RequestGroup),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RequestFriend {
    pub time: i64,
    pub self_id: i64,
    pub post_type: String,
    pub request_type: String,
    pub user_id: i64,
    pub comment: String,
    pub flag: String,
    #[serde(skip)]
    pub quick_action: Option<fn(Option<bool>)>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum RequestGroup {
    Add(RequestGroupAdd),
    Invite(RequestGroupInvite),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RequestGroupAdd {
    pub time: i64,
    pub self_id: i64,
    pub post_type: String,
    pub request_type: String,
    pub group_id: i64,
    pub user_id: i64,
    pub comment: String,
    pub flag: String,
    pub sub_type: String,
    #[serde(skip)]
    pub quick_action: Option<fn(Option<bool>, Option<String>)>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RequestGroupInvite {
    pub time: i64,
    pub self_id: i64,
    pub post_type: String,
    pub request_type: String,
    pub group_id: i64,
    pub user_id: i64,
    pub comment: String,
    pub flag: String,
    pub sub_type: String,
    #[serde(skip)]
    pub quick_action: Option<fn(Option<bool>, Option<String>)>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Notice {
    BotOffline(BotOffline),
    FriendAdd(FriendAdd),
    FriendRecall(FriendRecall),
    GroupAdmin(GroupAdmin),
    GroupBan(GroupBan),
    GroupCard(GroupCard),
    GroupDecrease(GroupDecrease),
    GroupIncrease(GroupIncrease),
    Essence(Essence),
    Notify(Notify),
    GroupRecall(GroupRecall),
    GroupUpload(GroupUpload),
    GroupMsgEmojiLike(GroupMsgEmojiLike),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BotOffline {
    pub time: i64,
    pub self_id: i64,
    pub post_type: String,
    pub notice_type: String,
    pub user_id: i64,
    pub tag: String,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FriendAdd {
    pub time: i64,
    pub self_id: i64,
    pub post_type: String,
    pub notice_type: String,
    pub user_id: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FriendRecall {
    pub time: i64,
    pub self_id: i64,
    pub post_type: String,
    pub notice_type: String,
    pub user_id: i64,
    pub message_id: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum GroupAdmin {
    Set(GroupAdminSet),
    Unset(GroupAdminUnset),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GroupAdminSet {
    pub time: i64,
    pub self_id: i64,
    pub post_type: String,
    pub group_id: i64,
    pub user_id: i64,
    pub notice_type: String,
    pub sub_type: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GroupAdminUnset {
    pub time: i64,
    pub self_id: i64,
    pub post_type: String,
    pub group_id: i64,
    pub user_id: i64,
    pub notice_type: String,
    pub sub_type: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum GroupBan {
    Ban(GroupBanBan),
    LiftBan(GroupBanLiftBan),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GroupBanBan {
    pub time: i64,
    pub self_id: i64,
    pub post_type: String,
    pub group_id: i64,
    pub user_id: i64,
    pub notice_type: String,
    pub operator_id: i64,
    pub duration: i64,
    pub sub_type: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GroupBanLiftBan {
    pub time: i64,
    pub self_id: i64,
    pub post_type: String,
    pub group_id: i64,
    pub user_id: i64,
    pub notice_type: String,
    pub operator_id: i64,
    pub duration: i64,
    pub sub_type: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GroupCard {
    pub time: i64,
    pub self_id: i64,
    pub post_type: String,
    pub group_id: i64,
    pub user_id: i64,
    pub notice_type: String,
    pub card_new: String,
    pub card_old: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum GroupDecrease {
    Leave(GroupDecreaseLeave),
    Kick(GroupDecreaseKick),
    KickMe(GroupDecreaseKickMe),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GroupDecreaseLeave {
    pub time: i64,
    pub self_id: i64,
    pub post_type: String,
    pub group_id: i64,
    pub user_id: i64,
    pub notice_type: String,
    pub sub_type: String,
    pub operator_id: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GroupDecreaseKick {
    pub time: i64,
    pub self_id: i64,
    pub post_type: String,
    pub group_id: i64,
    pub user_id: i64,
    pub notice_type: String,
    pub sub_type: String,
    pub operator_id: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GroupDecreaseKickMe {
    pub time: i64,
    pub self_id: i64,
    pub post_type: String,
    pub group_id: i64,
    pub user_id: i64,
    pub notice_type: String,
    pub sub_type: String,
    pub operator_id: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum GroupIncrease {
    Approve(GroupIncreaseApprove),
    Invite(GroupIncreaseInvite),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GroupIncreaseApprove {
    pub time: i64,
    pub self_id: i64,
    pub post_type: String,
    pub notice_type: String,
    pub sub_type: String,
    pub group_id: i64,
    pub operator_id: i64,
    pub user_id: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GroupIncreaseInvite {
    pub time: i64,
    pub self_id: i64,
    pub post_type: String,
    pub notice_type: String,
    pub sub_type: String,
    pub group_id: i64,
    pub operator_id: i64,
    pub user_id: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Essence {
    Add(GroupEssenceAdd),
    Delete(GroupEssenceDelete),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GroupEssenceAdd {
    pub time: i64,
    pub self_id: i64,
    pub post_type: String,
    pub notice_type: String,
    pub group_id: i64,
    pub user_id: i64,
    pub message_id: i64,
    pub sender_id: i64,
    pub sub_type: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GroupEssenceDelete {
    pub time: i64,
    pub self_id: i64,
    pub post_type: String,
    pub notice_type: String,
    pub group_id: i64,
    pub user_id: i64,
    pub message_id: i64,
    pub sender_id: i64,
    pub sub_type: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Notify {
    GroupName(NotifyGroupName),
    Title(NotifyTitle),
    InputStatus(InputStatus),
    Poke(Poke),
    ProfileLike(NotifyProfileLike),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NotifyGroupName {
    pub time: i64,
    pub self_id: i64,
    pub post_type: String,
    pub notice_type: String,
    pub group_id: i64,
    pub user_id: i64,
    pub sub_type: String,
    pub name_new: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NotifyTitle {
    pub time: i64,
    pub self_id: i64,
    pub post_type: String,
    pub notice_type: String,
    pub group_id: i64,
    pub user_id: i64,
    pub sub_type: String,
    pub title: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum InputStatus {
    Group(NotifyInputStatusGroup),
    Friend(NotifyInputStatusFriend),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NotifyInputStatusGroup {
    pub time: i64,
    pub self_id: i64,
    pub post_type: String,
    pub notice_type: String,
    pub sub_type: String,
    pub status_text: String,
    pub event_type: i64,
    pub user_id: i64,
    pub group_id: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NotifyInputStatusFriend {
    pub time: i64,
    pub self_id: i64,
    pub post_type: String,
    pub notice_type: String,
    pub sub_type: String,
    pub status_text: String,
    pub event_type: i64,
    pub user_id: i64,
    pub group_id: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Poke {
    Group(NotifyPokeGroup),
    Friend(NotifyPokeFriend),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NotifyPokeGroup {
    pub time: i64,
    pub self_id: i64,
    pub post_type: String,
    pub notice_type: String,
    pub sub_type: String,
    pub target_id: i64,
    pub user_id: i64,
    pub group_id: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NotifyPokeFriend {
    pub time: i64,
    pub self_id: i64,
    pub post_type: String,
    pub notice_type: String,
    pub sub_type: String,
    pub target_id: i64,
    pub user_id: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NotifyProfileLike {
    pub time: i64,
    pub self_id: i64,
    pub post_type: String,
    pub notice_type: String,
    pub sub_type: String,
    pub operator_id: i64,
    pub operator_nick: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GroupRecall {
    pub time: i64,
    pub self_id: i64,
    pub post_type: String,
    pub notice_type: String,
    pub group_id: i64,
    pub user_id: i64,
    pub operator_id: i64,
    pub message_id: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GroupUpload {
    pub time: i64,
    pub self_id: i64,
    pub post_type: String,
    pub notice_type: String,
    pub group_id: i64,
    pub user_id: i64,
    pub file: GroupUploadFile,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GroupUploadFile {
    pub id: String,
    pub name: String,
    pub size: i64,
    pub busid: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GroupMsgEmojiLike {
    pub time: i64,
    pub self_id: i64,
    pub post_type: String,
    pub notice_type: String,
    pub group_id: i64,
    pub user_id: i64,
    pub message_id: i64,
    pub likes: Vec<EmojiLike>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EmojiLike {
    pub emoji_id: String,
    pub count: i64,
}
