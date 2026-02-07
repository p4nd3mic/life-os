// Card states
export type CardState =
  | "pending"
  | "processing"
  | "awaiting_input"
  | "complete"
  | "error"
  | "cancelled";

// Card types
export type CardType =
  | "meal"
  | "delivery_order"
  | "delivery_session"
  | "media_add"
  | "music"
  | "thought"
  | "query"
  | "code_task"
  | "generic";

// Domain identifiers
export type DomainId =
  | "nutrition"
  | "delivery"
  | "media"
  | "youtube"
  | "finance"
  | "fitness"
  | "general";

// Image status
export type ImageStatus = "loading" | "ready" | "missing" | "upload_prompt";

// Layout mode for card rendering
export type StreamLayoutMode = "classic" | "cause_effect";

// Entity reference (linked entities in cards)
export type EntityRef = {
  type: string;
  id?: string;
  name: string;
  link?: string; // Obsidian wiki-link [[Entities/Food/Omelette]]
};

// Expanded card content
export type ExpandedSection = {
  title: string;
  body: string;
};

export type CardAction = {
  id: string;
  label: string;
  icon?: string;
  style?: string;
};

export type ExpandedContent = {
  originalInput?: string;
  sections: ExpandedSection[];
  entityLinks?: EntityLink[];
  actions: CardAction[];
};

// Image metadata
export type CardImage = {
  url?: string;
  status: ImageStatus;
  source?: string;
};

export type ImageCandidate = {
  sourcePath: string;
  sourceKind: string;
  score: number;
  reason: string[];
  fileName: string;
  isManaged: boolean;
};

export type ImageCandidateResponse = {
  entityKey: string;
  entityName: string;
  entityType: string;
  candidates: ImageCandidate[];
};

export type ImageAssetRecord = {
  id: string;
  relativePath: string;
  sourcePath: string;
  sourceKind: string;
  sha256: string;
  mime: string;
  createdAt: string;
  tags: string[];
};

export type ImageAttachResult = {
  patch: StreamCardPatch;
  version: number;
  entityKey: string;
  primaryRelativePath: string;
  asset: ImageAssetRecord;
};

export type ImageBackfillSummary = {
  updated: number;
  skipped: number;
  failed: number;
  errors: string[];
};

export type ImageAutoFetchMode = "review_first" | "auto_apply";

export type ImageAutoFetchSummary = {
  reviewed: number;
  applied: number;
  skipped: number;
  failed: number;
  errors: string[];
};

export type CardStatValue = string | number | boolean | null;

export type CardRequestMeta = {
  model?: string;
  effort?: string;
  accessMode?: string;
};

export type EntityLink = {
  name: string;
  path: string;
  icon?: string;
};

export type ClarificationOption = {
  id: string;
  label: string;
  emoji?: string;
};

export type CausalNodeRole =
  | "cause"
  | "effect"
  | "action"
  | "reward"
  | "question"
  | "response";

export type CausalNode = {
  id: string;
  text: string;
  headline?: string;
  summaryLine?: string;
  title?: string;
  bullets?: string[];
  details?: string;
  role?: CausalNodeRole;
  rank?: number;
  groupType?: "primary" | "overflow_summary";
  isImageApplicable?: boolean;
  image?: CardImage;
  entity?: EntityRef;
  occurredAt?: string;
};

export type CausalLink = {
  id?: string;
  fromId: string;
  toId: string;
  label?: string;
  strength?: number;
};

export type CausalLayoutState = {
  visibleRightCount?: number;
  topLinkLimit?: number;
  expanded?: boolean;
};

export type CausalSemanticMode =
  | "cause_effect"
  | "action_reward"
  | "statement_why"
  | "question_response";

export type CausalCompactionState = {
  enabled: boolean;
  threshold: number;
  overflowCount?: number;
};

export type CausalCardContent = {
  leftNodes: CausalNode[];
  rightNodes: CausalNode[];
  links: CausalLink[];
  layout?: CausalLayoutState;
  semanticMode?: CausalSemanticMode;
  compaction?: CausalCompactionState;
  transcriptSource?: "expanded" | "raw_response" | "both";
};

export type CausalRestructureAction =
  | "split_cause"
  | "merge_effects"
  | "relink_arrows"
  | "reframe_mode";

export type CausalRestructureResult = {
  patch: StreamCardPatch;
  version: number;
};

export type SemanticRegenerationResult = {
  updated: number;
  skipped: number;
  failed: number;
  errors: string[];
  llmLogStatus?: "success" | "failed";
};

export type DayThreadDebugLogItem = {
  cardId: string;
  threadId?: string;
  startedAt?: string;
  completedAt?: string;
  failureReason?: string;
  promptEchoDetected?: boolean;
  rawResponsePreview?: string;
  traceStats?: SemanticAttemptTraceStats;
  eventTracePreview?: SemanticEventTraceEntry[];
};

export type SemanticAttemptTraceStats = {
  eventCount: number;
  agentDeltaSeen: boolean;
  agentCompletedSeen: boolean;
  turnCompletedSeen: boolean;
  errorSeen: boolean;
  firstErrorMessage?: string;
  outputSource?: string;
  outputChars: number;
  methodsSeen: string[];
};

export type SemanticEventTraceEntry = {
  timestamp: string;
  method: string;
  normalizedMethod?: string;
  threadIds: string[];
  turnId?: string;
  itemType?: string;
  itemId?: string;
  paramsPreview?: string;
  deltaPreview?: string;
  errorPreview?: string;
};

export type DayThreadDebugSummary = {
  date: string;
  threadId?: string;
  lastSeedHash?: string;
  cardCount?: number;
  updatedAt?: string;
  logDirectory: string;
  totalLogs: number;
  failedLogs: number;
  successfulLogs: number;
  lastFailureReason?: string;
  lastFailureCardId?: string;
  recentLogs: DayThreadDebugLogItem[];
};

export type LifeStreamAuthHealth = {
  state: "healthy" | "unauthorized" | "unknown" | "error";
  message: string;
  checkedAt: string;
};

export type TaskDockItemKind = "task" | "reminder";
export type TaskDockFilter = "today" | "all";

export type TaskDockItem = {
  id: string;
  key: string;
  text: string;
  kind: TaskDockItemKind;
  completed: boolean;
  createdAt: string;
  updatedAt: string;
  targetDate: string;
  sourceCardId?: string;
  sourceNodeId?: string;
};

export type TaskDockPayload = {
  version: number;
  items: TaskDockItem[];
};

// Main StreamCard type
export type StreamCard = {
  id: string;
  occurredAt: string; // ISO timestamp - when event happened
  createdAt: string; // ISO timestamp - when card created
  updatedAt: string; // ISO timestamp - last update
  version: number; // For out-of-order event handling

  cardType: CardType;
  domain: DomainId;
  emoji: string;
  layoutMode?: StreamLayoutMode;
  causal?: CausalCardContent;

  state: CardState;
  processingStep?: string; // Current step being shown
  processingSteps?: string[]; // History of steps

  title: string;
  subtitle?: string;
  summary?: string;
  durationMs?: number;

  image?: CardImage;

  stats?: Record<string, CardStatValue>;
  entities?: EntityRef[];

  originalInput?: string;

  assistantPreview?: string;
  request?: CardRequestMeta;

  source?: {
    streamFile?: string;
    streamAnchor?: string;
  };

  expanded?: ExpandedContent;
  clarificationOptions?: ClarificationOption[];

  errorMessage?: string;
};

export type StreamCardPatch = {
  state?: CardState;
  title?: string;
  subtitle?: string;
  processingStep?: string;
  processingSteps?: string[];
  durationMs?: number;
  errorMessage?: string;
  assistantPreview?: string;
  stats?: Record<string, CardStatValue>;
  image?: CardImage;
  expanded?: ExpandedContent;
  clarificationOptions?: ClarificationOption[];
  layoutMode?: StreamLayoutMode;
  causal?: CausalCardContent;
};

// Event types for real-time updates
export type LifeStreamEvent =
  | { type: "card_created"; card: StreamCard }
  | { type: "card_step"; cardId: string; step: string; version: number }
  | {
      type: "card_updated";
      cardId: string;
      patch: StreamCardPatch;
      version: number;
    }
  | { type: "card_completed"; card: StreamCard }
  | { type: "card_error"; cardId: string; message: string; version: number };

// Filter state
export type StreamFilters = {
  date: string; // ISO date (YYYY-MM-DD)
  domains: Set<DomainId>;
};
