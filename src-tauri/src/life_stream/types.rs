use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CardState {
    Pending,
    Processing,
    AwaitingInput,
    Complete,
    Error,
    Cancelled,
}

impl CardState {
    pub fn can_transition_to(&self, next: &CardState) -> bool {
        use CardState::*;
        matches!(
            (self, next),
            (Pending, Processing)
                | (Processing, Complete)
                | (Processing, Error)
                | (Processing, AwaitingInput)
                | (Processing, Cancelled)
                | (AwaitingInput, Processing)
                | (AwaitingInput, Cancelled)
                | (Error, Processing)
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CardType {
    Meal,
    DeliveryOrder,
    DeliverySession,
    MediaAdd,
    Music,
    Thought,
    Query,
    CodeTask,
    Generic,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DomainId {
    Nutrition,
    Delivery,
    Media,
    Youtube,
    Finance,
    Fitness,
    General,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum LayoutMode {
    #[default]
    Classic,
    CauseEffect,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ImageStatus {
    Loading,
    Ready,
    Missing,
    UploadPrompt,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardImage {
    pub url: Option<String>,
    pub status: ImageStatus,
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageCandidate {
    #[serde(rename = "sourcePath")]
    pub source_path: String,
    #[serde(rename = "sourceKind")]
    pub source_kind: String,
    pub score: i64,
    #[serde(default)]
    pub reason: Vec<String>,
    #[serde(rename = "fileName")]
    pub file_name: String,
    #[serde(rename = "isManaged")]
    pub is_managed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageCandidateResponse {
    #[serde(rename = "entityKey")]
    pub entity_key: String,
    #[serde(rename = "entityName")]
    pub entity_name: String,
    #[serde(rename = "entityType")]
    pub entity_type: String,
    pub candidates: Vec<ImageCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageAssetRecord {
    pub id: String,
    #[serde(rename = "relativePath")]
    pub relative_path: String,
    #[serde(rename = "sourcePath")]
    pub source_path: String,
    #[serde(rename = "sourceKind")]
    pub source_kind: String,
    pub sha256: String,
    pub mime: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageAttachResult {
    pub patch: StreamCardPatch,
    pub version: u32,
    #[serde(rename = "entityKey")]
    pub entity_key: String,
    #[serde(rename = "primaryRelativePath")]
    pub primary_relative_path: String,
    pub asset: ImageAssetRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageBackfillSummary {
    pub updated: usize,
    pub skipped: usize,
    pub failed: usize,
    #[serde(default)]
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ImageAutoFetchMode {
    ReviewFirst,
    AutoApply,
}

impl Default for ImageAutoFetchMode {
    fn default() -> Self {
        Self::ReviewFirst
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageAutoFetchSummary {
    pub reviewed: usize,
    pub applied: usize,
    pub skipped: usize,
    pub failed: usize,
    #[serde(default)]
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityRef {
    #[serde(rename = "type")]
    pub entity_type: String,
    pub id: Option<String>,
    pub name: String,
    pub link: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpandedSection {
    pub title: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardAction {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub style: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpandedContent {
    #[serde(rename = "originalInput")]
    pub original_input: Option<String>,
    pub sections: Vec<ExpandedSection>,
    #[serde(rename = "entityLinks")]
    pub entity_links: Option<Vec<EntityLink>>,
    pub actions: Vec<CardAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityLink {
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClarificationOption {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub emoji: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CausalNodeRole {
    Cause,
    Effect,
    Action,
    Reward,
    Question,
    Response,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CausalNode {
    pub id: String,
    pub text: String,
    #[serde(default)]
    pub headline: Option<String>,
    #[serde(default, rename = "summaryLine")]
    pub summary_line: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub bullets: Option<Vec<String>>,
    #[serde(default)]
    pub details: Option<String>,
    #[serde(default)]
    pub role: Option<CausalNodeRole>,
    #[serde(default)]
    pub rank: Option<u32>,
    #[serde(default, rename = "groupType")]
    pub group_type: Option<CausalGroupType>,
    #[serde(default = "default_true", rename = "isImageApplicable")]
    pub is_image_applicable: bool,
    #[serde(default)]
    pub image: Option<CardImage>,
    #[serde(default)]
    pub entity: Option<EntityRef>,
    #[serde(default, rename = "occurredAt")]
    pub occurred_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CausalLink {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(rename = "fromId")]
    pub from_id: String,
    #[serde(rename = "toId")]
    pub to_id: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub strength: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CausalLayoutState {
    #[serde(default, rename = "visibleRightCount")]
    pub visible_right_count: Option<u32>,
    #[serde(default, rename = "topLinkLimit")]
    pub top_link_limit: Option<u32>,
    #[serde(default)]
    pub expanded: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CausalSemanticMode {
    CauseEffect,
    ActionReward,
    StatementWhy,
    QuestionResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CausalGroupType {
    Primary,
    OverflowSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CausalCompactionState {
    pub enabled: bool,
    pub threshold: u32,
    #[serde(default, rename = "overflowCount")]
    pub overflow_count: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CausalCardContent {
    #[serde(rename = "leftNodes")]
    pub left_nodes: Vec<CausalNode>,
    #[serde(rename = "rightNodes")]
    pub right_nodes: Vec<CausalNode>,
    pub links: Vec<CausalLink>,
    #[serde(default)]
    pub layout: Option<CausalLayoutState>,
    #[serde(default, rename = "semanticMode")]
    pub semantic_mode: Option<CausalSemanticMode>,
    #[serde(default)]
    pub compaction: Option<CausalCompactionState>,
    #[serde(default, rename = "transcriptSource")]
    pub transcript_source: Option<CausalTranscriptSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CausalTranscriptSource {
    Expanded,
    RawResponse,
    Both,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardSource {
    #[serde(rename = "streamFile")]
    pub stream_file: Option<String>,
    #[serde(rename = "streamAnchor")]
    pub stream_anchor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardRequestMeta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "accessMode")]
    pub access_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamCard {
    pub id: String,
    #[serde(rename = "occurredAt")]
    pub occurred_at: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    pub version: u32,

    #[serde(rename = "cardType")]
    pub card_type: CardType,
    pub domain: DomainId,
    pub emoji: String,
    #[serde(default, rename = "layoutMode")]
    pub layout_mode: LayoutMode,
    #[serde(default)]
    pub causal: Option<CausalCardContent>,

    pub state: CardState,
    #[serde(rename = "processingStep")]
    pub processing_step: Option<String>,
    #[serde(rename = "processingSteps")]
    pub processing_steps: Option<Vec<String>>,

    pub title: String,
    pub subtitle: Option<String>,
    pub summary: Option<String>,
    #[serde(rename = "durationMs")]
    pub duration_ms: Option<u64>,

    pub image: Option<CardImage>,

    pub stats: Option<HashMap<String, CardStatValue>>,
    pub entities: Option<Vec<EntityRef>>,

    #[serde(rename = "originalInput")]
    pub original_input: Option<String>,

    #[serde(rename = "assistantPreview")]
    pub assistant_preview: Option<String>,

    pub request: Option<CardRequestMeta>,

    pub source: Option<CardSource>,

    pub expanded: Option<ExpandedContent>,

    #[serde(rename = "clarificationOptions")]
    pub clarification_options: Option<Vec<ClarificationOption>>,

    #[serde(rename = "errorMessage")]
    pub error_message: Option<String>,
}

// Event types for broadcasting
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LifeStreamEvent {
    CardCreated {
        card: StreamCard,
    },
    CardStep {
        #[serde(rename = "cardId")]
        card_id: String,
        step: String,
        version: u32,
    },
    CardUpdated {
        #[serde(rename = "cardId")]
        card_id: String,
        patch: StreamCardPatch,
        version: u32,
    },
    CardCompleted {
        card: StreamCard,
    },
    CardError {
        #[serde(rename = "cardId")]
        card_id: String,
        message: String,
        version: u32,
    },
}

/// Typed patch for card updates via events.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StreamCardPatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<CardState>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none", rename = "processingStep")]
    pub processing_step: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none", rename = "processingSteps")]
    pub processing_steps: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none", rename = "durationMs")]
    pub duration_ms: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none", rename = "errorMessage")]
    pub error_message: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none", rename = "assistantPreview")]
    pub assistant_preview: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub stats: Option<HashMap<String, CardStatValue>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<CardImage>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub expanded: Option<ExpandedContent>,

    #[serde(skip_serializing_if = "Option::is_none", rename = "layoutMode")]
    pub layout_mode: Option<LayoutMode>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub causal: Option<CausalCardContent>,

    #[serde(
        skip_serializing_if = "Option::is_none",
        rename = "clarificationOptions"
    )]
    pub clarification_options: Option<Vec<ClarificationOption>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CausalRestructureAction {
    SplitCause,
    MergeEffects,
    RelinkArrows,
    ReframeMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CausalRestructureResult {
    pub patch: StreamCardPatch,
    pub version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticRegenerationResult {
    pub updated: usize,
    pub skipped: usize,
    pub failed: usize,
    #[serde(default)]
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TaskDockItemKind {
    Task,
    Reminder,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskDockItem {
    pub id: String,
    pub key: String,
    pub text: String,
    pub kind: TaskDockItemKind,
    pub completed: bool,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    #[serde(rename = "targetDate")]
    pub target_date: String,
    #[serde(default, rename = "sourceCardId")]
    pub source_card_id: Option<String>,
    #[serde(default, rename = "sourceNodeId")]
    pub source_node_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskDockPayload {
    pub version: u32,
    #[serde(default)]
    pub items: Vec<TaskDockItem>,
}

impl Default for TaskDockPayload {
    fn default() -> Self {
        Self {
            version: 1,
            items: Vec::new(),
        }
    }
}

/// Constrained stat value type for sync with TypeScript.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CardStatValue {
    String(String),
    Integer(i64),
    Float(f64),
    Bool(bool),
    Null,
}

#[derive(Debug, Error)]
pub enum LifeStreamError {
    #[error("Configuration error: {0}")]
    Configuration(String),
    #[error("Security violation: {0}")]
    Security(String),
    #[error("I/O error: {0}")]
    Io(String),
    #[error("Parse error: {0}")]
    Parse(String),
}

// Submit input parameters
#[derive(Debug, Clone, Deserialize)]
pub struct SubmitInput {
    #[serde(rename = "workspaceId")]
    pub workspace_id: String,
    #[serde(rename = "cardId")]
    pub card_id: String,
    pub input: String,
    #[serde(rename = "occurredAtIso")]
    pub occurred_at_iso: Option<String>,
}
