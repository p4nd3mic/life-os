import {
  createContext,
  useCallback,
  useEffect,
  useContext,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
  type ReactNode,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { useLifeStream } from "../hooks/useLifeStream";
import { taskDockStore } from "../state/taskDockStore";
import type {
  CausalRestructureAction,
  DayThreadDebugSummary,
  DomainId,
  EntityRef,
  LifeStreamAuthHealth,
  ImageAutoFetchMode,
  ImageAutoFetchSummary,
  ImageAttachResult,
  ImageCandidateResponse,
  SemanticRegenerationResult,
  StreamCard,
  TaskDockFilter,
  TaskDockItem,
  TaskDockPayload,
} from "../types";

const FILTER_STORAGE_KEY = "life-stream-filters";

type LifeStreamContextValue = {
  cards: StreamCard[];
  filteredCards: StreamCard[];
  isLoading: boolean;
  loadError: string | null;
  submitStatus: {
    state: "idle" | "sending" | "received" | "created" | "error";
    message?: string;
  };
  clearSubmitStatus: () => void;
  workspaceId: string | null;

  currentDate: string;
  goToPreviousDay: () => void;
  goToNextDay: () => void;
  goToToday: () => void;

  activeFilters: Set<DomainId>;
  toggleFilter: (domain: DomainId) => void;
  clearFilters: () => void;

  submit: (input: string, options?: LifeStreamSubmitOptions) => Promise<void>;
  cancel: (cardId: string) => Promise<void>;
  retry: (cardId: string) => Promise<void>;
  clarify: (cardId: string, optionId: string) => Promise<void>;
  restructure: (
    cardId: string,
    action: CausalRestructureAction,
    options?: { sourceNodeIds?: string[]; targetMode?: "cause_effect" | "action_reward" },
  ) => Promise<void>;
  getImageCandidates: (
    cardId: string,
    nodeId?: string | null,
  ) => Promise<ImageCandidateResponse | null>;
  attachImage: (
    cardId: string,
    sourcePath: string,
    options?: {
      nodeId?: string | null;
      setPrimary?: boolean;
      setContextOverride?: boolean;
      contextHint?: string | null;
      updateEntityFile?: boolean;
      updateEntityEmbed?: boolean;
    },
  ) => Promise<ImageAttachResult | null>;
  semanticRegenerationStatus: {
    state: "idle" | "running" | "done" | "error";
    message?: string;
  };
  regenerateSemanticsForCurrentDate: () => Promise<SemanticRegenerationResult | null>;
  clearSemanticRegenerationStatus: () => void;
  dayThreadDebugStatus: {
    state: "idle" | "running" | "done" | "error";
    message?: string;
    details?: DayThreadDebugSummary;
  };
  inspectDayThreadForCurrentDate: () => Promise<DayThreadDebugSummary | null>;
  resetDayThreadAndRebuildForCurrentDate: () => Promise<SemanticRegenerationResult | null>;
  clearDayThreadDebugStatus: () => void;
  authHealthStatus: {
    state: "idle" | "checking" | "healthy" | "unauthorized" | "unknown" | "error";
    message?: string;
    details?: LifeStreamAuthHealth;
  };
  checkAuthHealth: () => Promise<LifeStreamAuthHealth | null>;
  imageAutoFetchStatus: {
    state: "idle" | "running" | "done" | "error";
    message?: string;
  };
  autoFetchImagesForCurrentDate: (
    mode?: ImageAutoFetchMode,
  ) => Promise<ImageAutoFetchSummary | null>;
  clearImageAutoFetchStatus: () => void;

  taskDock: {
    items: TaskDockItem[];
    filter: TaskDockFilter;
    collapsed: boolean;
    hidden: boolean;
  };
  addTaskDockItem: (text: string) => void;
  toggleTaskDockItem: (id: string) => void;
  moveTaskDockItem: (id: string, direction: -1 | 1, visibleIds: string[]) => void;
  setTaskDockFilter: (filter: TaskDockFilter) => void;
  setTaskDockCollapsed: (collapsed: boolean) => void;
  setTaskDockHidden: (hidden: boolean) => void;
};

type LifeStreamSubmitOptions = {
  occurredAtIso?: string;
  modelId?: string | null;
  effort?: string | null;
  accessMode?: "read-only" | "current" | "full-access";
  collaborationMode?: Record<string, unknown> | null;
};

const LifeStreamContext = createContext<LifeStreamContextValue | null>(null);

type LifeStreamProviderProps = {
  workspaceId: string | null;
  children: ReactNode;
};

export function LifeStreamProvider({
  workspaceId,
  children,
}: LifeStreamProviderProps) {
  const {
    cards,
    isLoading,
    loadError,
    currentDate,
    submit,
    submitStatus,
    clearSubmitStatus,
    cancel,
    retry,
    clarify,
    restructure,
    regenerateSemantics,
    getDayThreadDebug,
    getAuthHealth,
    resetDayThreadAndRebuild,
    getImageCandidates,
    attachImage,
    autoFetchImages,
    goToPreviousDay,
    goToNextDay,
    goToToday,
  } = useLifeStream(workspaceId);
  const saveTimerRef = useRef<number | null>(null);

  const [activeFilters, setActiveFilters] = useState<Set<DomainId>>(() => {
    if (typeof window === "undefined") {
      return new Set();
    }
    try {
      const stored = window.localStorage.getItem(FILTER_STORAGE_KEY);
      if (!stored) return new Set();
      const parsed = JSON.parse(stored) as DomainId[];
      return new Set(parsed);
    } catch {
      return new Set();
    }
  });
  const [semanticRegenerationStatus, setSemanticRegenerationStatus] = useState<{
    state: "idle" | "running" | "done" | "error";
    message?: string;
  }>({ state: "idle" });
  const [dayThreadDebugStatus, setDayThreadDebugStatus] = useState<{
    state: "idle" | "running" | "done" | "error";
    message?: string;
    details?: DayThreadDebugSummary;
  }>({ state: "idle" });
  const [authHealthStatus, setAuthHealthStatus] = useState<{
    state: "idle" | "checking" | "healthy" | "unauthorized" | "unknown" | "error";
    message?: string;
    details?: LifeStreamAuthHealth;
  }>({ state: "idle" });
  const [imageAutoFetchStatus, setImageAutoFetchStatus] = useState<{
    state: "idle" | "running" | "done" | "error";
    message?: string;
  }>({ state: "idle" });
  const semanticStatusTimerRef = useRef<number | null>(null);
  const imageAutoFetchStatusTimerRef = useRef<number | null>(null);
  const authHealthInFlightRef = useRef(false);

  const persistFilters = useCallback((next: Set<DomainId>) => {
    setActiveFilters(next);
    if (typeof window !== "undefined") {
      try {
        window.localStorage.setItem(
          FILTER_STORAGE_KEY,
          JSON.stringify(Array.from(next))
        );
      } catch {
        // ignore storage errors
      }
    }
  }, []);

  const toggleFilter = useCallback(
    (domain: DomainId) => {
      const next = new Set(activeFilters);
      if (next.has(domain)) {
        next.delete(domain);
      } else {
        next.add(domain);
      }
      persistFilters(next);
    },
    [activeFilters, persistFilters]
  );

  const clearFilters = useCallback(() => {
    persistFilters(new Set());
  }, [persistFilters]);

  const filteredCards =
    activeFilters.size === 0
      ? cards
      : cards.filter((card) => activeFilters.has(card.domain));

  const clearSemanticRegenerationStatus = useCallback(() => {
    setSemanticRegenerationStatus({ state: "idle" });
  }, []);

  const clearDayThreadDebugStatus = useCallback(() => {
    setDayThreadDebugStatus({ state: "idle" });
  }, []);

  const scheduleSemanticStatusClear = useCallback((delayMs = 3200) => {
    if (semanticStatusTimerRef.current) {
      window.clearTimeout(semanticStatusTimerRef.current);
    }
    semanticStatusTimerRef.current = window.setTimeout(() => {
      setSemanticRegenerationStatus({ state: "idle" });
      semanticStatusTimerRef.current = null;
    }, delayMs);
  }, []);

  useEffect(() => {
    return () => {
      if (semanticStatusTimerRef.current) {
        window.clearTimeout(semanticStatusTimerRef.current);
        semanticStatusTimerRef.current = null;
      }
      if (imageAutoFetchStatusTimerRef.current) {
        window.clearTimeout(imageAutoFetchStatusTimerRef.current);
        imageAutoFetchStatusTimerRef.current = null;
      }
    };
  }, []);

  const regenerateSemanticsForCurrentDate = useCallback(async () => {
    if (!workspaceId) {
      return null;
    }
    const cardIds = cards.map((card) => card.id);
    if (cardIds.length === 0) {
      setSemanticRegenerationStatus({
        state: "done",
        message: "🧠 Nothing to rebuild on this date.",
      });
      scheduleSemanticStatusClear();
      return { updated: 0, skipped: 0, failed: 0, errors: [] };
    }

    setSemanticRegenerationStatus({
      state: "running",
      message: "🧠 LLM rewrite in progress…",
    });

    const result = await regenerateSemantics(cardIds, {
      forceLlm: true,
      persist: true,
    });
    if (!result) {
      setSemanticRegenerationStatus({
        state: "error",
        message: "⚠️ Failed to regenerate semantics.",
      });
      return null;
    }

    const hasRetainedPreviousNodes = result.errors.some((error) =>
      error.toLowerCase().includes("llm output invalid, retained previous nodes")
    );
    const authError = result.errors.find((error) =>
      isSemanticAuthError(error)
    );
    if (authError) {
      setAuthHealthStatus({
        state: "unauthorized",
        message: "🔴 Codex auth invalid for Life workspace",
        details: {
          state: "unauthorized",
          message: authError,
          checkedAt: new Date().toISOString(),
        },
      });
    }
    const llmLogStatusSuffix =
      result.llmLogStatus === "success"
        ? " · 🧾 LLM log saved"
        : result.llmLogStatus === "failed"
          ? " · ⚠️ LLM log failed"
          : "";

    if (result.failed > 0 && result.updated === 0) {
      const message = authError
        ? `🔐 Rebuild blocked: Codex auth failed in Life workspace (401 / missing bearer token). Run \`codex login\` in /Volumes/YouTube 4TB/Life, then rebuild.${llmLogStatusSuffix}`
        : hasRetainedPreviousNodes
          ? `⚠️ LLM output invalid, retained previous nodes.${llmLogStatusSuffix}`
          : (() => {
              const firstError = result.errors[0];
              const base = firstError
                ? `⚠️ Rebuild failed (0 updated, ${result.failed} failed): ${firstError}`
                : `⚠️ Rebuild failed (0 updated, ${result.failed} failed).`;
              return `${base}${llmLogStatusSuffix}`;
            })();
      setSemanticRegenerationStatus({ state: "error", message });
      return result;
    }

    if (result.failed > 0) {
      const message = authError
        ? `🔐 Partial rebuild blocked by Codex auth (401 / missing bearer token). Re-auth with \`codex login\` in /Volumes/YouTube 4TB/Life.${llmLogStatusSuffix}`
        : hasRetainedPreviousNodes
          ? `🟡 LLM output invalid, retained previous nodes on ${result.failed} card(s) · rebuilt ${result.updated}${llmLogStatusSuffix}`
          : `🟡 Rebuilt ${result.updated} · failed ${result.failed}${llmLogStatusSuffix}`;
      setSemanticRegenerationStatus({ state: "done", message });
      return result;
    }

    const message = `✅ Rebuilt ${result.updated}${llmLogStatusSuffix}`;
    setSemanticRegenerationStatus({ state: "done", message });
    scheduleSemanticStatusClear(3200);
    return result;
  }, [cards, regenerateSemantics, scheduleSemanticStatusClear, workspaceId]);

  const inspectDayThreadForCurrentDate = useCallback(async () => {
    if (!workspaceId) {
      return null;
    }
    setDayThreadDebugStatus({
      state: "running",
      message: "🧪 Inspecting day-thread runtime + logs…",
    });

    const summary = await getDayThreadDebug(currentDate);
    if (!summary) {
      setDayThreadDebugStatus({
        state: "error",
        message: "⚠️ Unable to read day-thread debug state.",
      });
      return null;
    }

    const promptEchoCount = summary.recentLogs.filter(
      (item) => item.promptEchoDetected
    ).length;
    const latestLog = summary.recentLogs[0];
    const traceStats = latestLog?.traceStats;
    const traceFlags = traceStats
      ? ` · trace Δ${traceStats.agentDeltaSeen ? "✅" : "❌"} msg${traceStats.agentCompletedSeen ? "✅" : "❌"} err${traceStats.errorSeen ? "✅" : "❌"} done${traceStats.turnCompletedSeen ? "✅" : "❌"}`
      : "";
    const threadShort = summary.threadId
      ? `${summary.threadId.slice(0, 10)}…`
      : "none";
    const failureSource =
      summary.lastFailureReason ?? traceStats?.firstErrorMessage;
    const failureSnippet = failureSource
      ? ` · last fail: ${failureSource.slice(0, 96)}`
      : "";
    const failureCardSnippet = summary.lastFailureCardId
      ? ` · card ${summary.lastFailureCardId.slice(0, 18)}…`
      : "";
    const promptEchoSnippet =
      promptEchoCount > 0 ? ` · ⚠️ prompt-echo ${promptEchoCount}` : "";
    const traceActionSnippet = latestLog ? " · Open trace summary" : "";
    const message = `🧪 ${summary.date} · thread ${threadShort} · logs ${summary.totalLogs} (✅${summary.successfulLogs}/❌${summary.failedLogs})${traceFlags}${promptEchoSnippet}${failureCardSnippet}${failureSnippet}${traceActionSnippet}`;

    setDayThreadDebugStatus({
      state: "done",
      message,
      details: summary,
    });
    return summary;
  }, [currentDate, getDayThreadDebug, workspaceId]);

  const resetDayThreadAndRebuildForCurrentDate = useCallback(async () => {
    if (!workspaceId) {
      return null;
    }
    setDayThreadDebugStatus({
      state: "running",
      message: "♻️ Resetting day-thread and rebuilding semantics…",
    });
    const result = await resetDayThreadAndRebuild(currentDate, true);
    if (!result) {
      setDayThreadDebugStatus({
        state: "error",
        message: "⚠️ Reset day-thread + rebuild failed.",
      });
      return null;
    }
    setDayThreadDebugStatus({
      state: result.failed > 0 ? "error" : "done",
      message:
        result.failed > 0
          ? `⚠️ Reset complete, rebuilt ${result.updated}, failed ${result.failed}`
          : `✅ Reset complete, rebuilt ${result.updated}`,
    });
    return result;
  }, [currentDate, resetDayThreadAndRebuild, workspaceId]);

  const checkAuthHealth = useCallback(async () => {
    if (!workspaceId) {
      return null;
    }
    if (authHealthInFlightRef.current) {
      return null;
    }
    authHealthInFlightRef.current = true;
    setAuthHealthStatus({
      state: "checking",
      message: "🩺 Checking Codex auth…",
    });
    try {
      const result = await getAuthHealth();
      if (!result) {
        setAuthHealthStatus({
          state: "error",
          message: "⚠️ Auth health check failed.",
        });
        return null;
      }
      const nextState =
        result.state === "healthy"
          ? "healthy"
          : result.state === "unauthorized"
            ? "unauthorized"
            : result.state === "unknown"
              ? "unknown"
              : "error";
      const prefix =
        nextState === "healthy"
          ? "🟢"
          : nextState === "unauthorized"
            ? "🔴"
            : nextState === "unknown"
              ? "🟡"
              : "⚠️";
      setAuthHealthStatus({
        state: nextState,
        message: `${prefix} ${result.message}`,
        details: result,
      });
      return result;
    } finally {
      authHealthInFlightRef.current = false;
    }
  }, [getAuthHealth, workspaceId]);

  useEffect(() => {
    setAuthHealthStatus({ state: "idle" });
  }, [workspaceId]);

  const clearImageAutoFetchStatus = useCallback(() => {
    setImageAutoFetchStatus({ state: "idle" });
  }, []);

  const scheduleImageAutoFetchStatusClear = useCallback((delayMs = 3600) => {
    if (imageAutoFetchStatusTimerRef.current) {
      window.clearTimeout(imageAutoFetchStatusTimerRef.current);
    }
    imageAutoFetchStatusTimerRef.current = window.setTimeout(() => {
      setImageAutoFetchStatus({ state: "idle" });
      imageAutoFetchStatusTimerRef.current = null;
    }, delayMs);
  }, []);

  const autoFetchImagesForCurrentDate = useCallback(
    async (mode: ImageAutoFetchMode = "review_first") => {
      if (!workspaceId) {
        return null;
      }
      const cardIds = cards.map((card) => card.id);
      if (cardIds.length === 0) {
        setImageAutoFetchStatus({
          state: "done",
          message: "🖼️ No cards to scan on this date.",
        });
        scheduleImageAutoFetchStatusClear();
        return {
          reviewed: 0,
          applied: 0,
          skipped: 0,
          failed: 0,
          errors: [],
        };
      }

      setImageAutoFetchStatus({
        state: "running",
        message:
          mode === "auto_apply"
            ? "🖼️ Auto-applying top image candidates…"
            : "🖼️ Fetching image candidates for review…",
      });

      const result = await autoFetchImages(cardIds, {
        mode,
        updateEntityFile: true,
        updateEntityEmbed: false,
      });
      if (!result) {
        setImageAutoFetchStatus({
          state: "error",
          message: "⚠️ Image auto-fetch failed.",
        });
        scheduleImageAutoFetchStatusClear(4600);
        return null;
      }

      if (result.failed > 0) {
        setImageAutoFetchStatus({
          state: "error",
          message: `⚠️ Images: ${result.applied} applied · ${result.reviewed} queued · ${result.failed} failed`,
        });
        scheduleImageAutoFetchStatusClear(5200);
        return result;
      }

      const message =
        mode === "auto_apply"
          ? `✅ Applied ${result.applied} · skipped ${result.skipped}`
          : `✅ Queued ${result.reviewed} review tasks · skipped ${result.skipped}`;
      setImageAutoFetchStatus({ state: "done", message });
      scheduleImageAutoFetchStatusClear();
      return result;
    },
    [autoFetchImages, cards, scheduleImageAutoFetchStatusClear, workspaceId],
  );

  const taskDock = useSyncExternalStore(
    (listener) => taskDockStore.subscribe(listener),
    () => taskDockStore.getSnapshot(),
    () => taskDockStore.getSnapshot(),
  );

  useEffect(() => {
    if (!workspaceId) {
      taskDockStore.hydrate({ version: 1, items: [] });
      return;
    }

    let cancelled = false;
    void (async () => {
      try {
        const payload = await invoke<TaskDockPayload>("life_stream_task_dock_load", {
          workspaceId,
        });
        if (!cancelled) {
          taskDockStore.hydrate(payload);
        }
      } catch (error) {
        console.error("Failed to load task dock:", error);
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [workspaceId]);

  useEffect(() => {
    if (!workspaceId) return;

    if (saveTimerRef.current) {
      window.clearTimeout(saveTimerRef.current);
    }
    saveTimerRef.current = window.setTimeout(() => {
      void invoke("life_stream_task_dock_save", {
        workspaceId,
        payload: taskDockStore.getPayload(),
      }).catch((error) => {
        console.error("Failed to save task dock:", error);
      });
      saveTimerRef.current = null;
    }, 280);

    return () => {
      if (saveTimerRef.current) {
        window.clearTimeout(saveTimerRef.current);
        saveTimerRef.current = null;
      }
    };
  }, [taskDock.items, workspaceId]);

  const inferredReminders = useMemo(
    () => deriveImageReminders(cards, currentDate),
    [cards, currentDate],
  );

  useEffect(() => {
    if (inferredReminders.length === 0) return;
    taskDockStore.upsertInferredReminders(inferredReminders);
  }, [inferredReminders]);

  const addTaskDockItem = useCallback(
    (text: string) => {
      taskDockStore.addManualTask(text, currentDate);
    },
    [currentDate],
  );

  const toggleTaskDockItem = useCallback((id: string) => {
    taskDockStore.toggleComplete(id);
  }, []);

  const moveTaskDockItem = useCallback(
    (id: string, direction: -1 | 1, visibleIds: string[]) => {
      taskDockStore.moveByVisibleIds(id, direction, visibleIds);
    },
    [],
  );

  const setTaskDockFilter = useCallback((filter: TaskDockFilter) => {
    taskDockStore.setFilter(filter);
  }, []);

  const setTaskDockCollapsed = useCallback((collapsed: boolean) => {
    taskDockStore.setCollapsed(collapsed);
  }, []);

  const setTaskDockHidden = useCallback((hidden: boolean) => {
    taskDockStore.setHidden(hidden);
  }, []);

  const value: LifeStreamContextValue = {
    cards,
    filteredCards,
    isLoading,
    loadError,
    submitStatus,
    clearSubmitStatus,
    workspaceId,
    currentDate,
    goToPreviousDay,
    goToNextDay,
    goToToday,
    activeFilters,
    toggleFilter,
    clearFilters,
    submit,
    cancel,
    retry,
    clarify,
    restructure,
    semanticRegenerationStatus,
    regenerateSemanticsForCurrentDate,
    clearSemanticRegenerationStatus,
    dayThreadDebugStatus,
    inspectDayThreadForCurrentDate,
    resetDayThreadAndRebuildForCurrentDate,
    clearDayThreadDebugStatus,
    authHealthStatus,
    checkAuthHealth,
    imageAutoFetchStatus,
    autoFetchImagesForCurrentDate,
    clearImageAutoFetchStatus,
    getImageCandidates,
    attachImage,
    taskDock,
    addTaskDockItem,
    toggleTaskDockItem,
    moveTaskDockItem,
    setTaskDockFilter,
    setTaskDockCollapsed,
    setTaskDockHidden,
  };

  return (
    <LifeStreamContext.Provider value={value}>
      {children}
    </LifeStreamContext.Provider>
  );
}

function isSemanticAuthError(value: string): boolean {
  const normalized = value.toLowerCase();
  return (
    normalized.includes("missing bearer") ||
    normalized.includes("unauthorized") ||
    normalized.includes("authentication in header") ||
    (normalized.includes("401") && normalized.includes("api.openai.com"))
  );
}

const IMAGE_CAPABLE_TYPES = new Set([
  "media",
  "anime",
  "movie",
  "show",
  "tv",
  "game",
  "people",
  "person",
  "food",
  "meal",
  "delivery",
  "fitness",
  "finance",
  "youtube",
  "geography",
  "project",
  "general",
  "note",
]);

function normalizeEntityType(value: string | undefined): string {
  return (value ?? "")
    .toLowerCase()
    .replace(/^entities\//, "")
    .replace(/^entity\//, "")
    .replace(/\s+/g, "");
}

function typeFolderForReminder(type: string): string {
  switch (type) {
    case "media":
    case "anime":
    case "movie":
    case "show":
    case "tv":
    case "game":
      return "Media";
    case "people":
    case "person":
      return "People";
    case "food":
    case "meal":
      return "Food";
    case "delivery":
      return "Delivery";
    case "fitness":
      return "Fitness";
    case "finance":
      return "Finance";
    case "youtube":
      return "YouTube";
    case "geography":
      return "Topics";
    case "project":
      return "Projects";
    default:
      return "Notes";
  }
}

function parseEntityFromText(text: string): { type: string; name: string; link: string } | null {
  const wikiLink = text.match(/\[\[\s*([^\]]+)\s*\]\]/);
  if (!wikiLink?.[1]) {
    return null;
  }
  const raw = wikiLink[1].trim();
  const canonical = raw.split("|")[0]?.trim() ?? raw;
  const parts = canonical.split("/");
  const last = parts.length > 0 ? parts[parts.length - 1]?.trim() : "";
  if (!last) {
    return null;
  }
  const inferredType = parts.length > 1 ? normalizeEntityType(parts[parts.length - 2]) : "general";
  return {
    type: inferredType,
    name: last,
    link: `[[${canonical}]]`,
  };
}

function resolveEntityReminderPayload(
  entity: EntityRef | null | undefined,
  fallbackText: string,
): { type: string; name: string; link: string } | null {
  if (entity) {
    const type = normalizeEntityType(entity.type);
    const name = entity.name?.trim() || "";
    const fromLink = entity.link
      ?.replace("[[", "")
      .replace("]]", "")
      .split("|")[0]
      ?.trim();
    const link = fromLink
      ? `[[${fromLink}]]`
      : name
      ? `[[Entities/${typeFolderForReminder(type)}/${name}]]`
      : "";
    if (!name) {
      return parseEntityFromText(fallbackText);
    }
    return {
      type,
      name,
      link,
    };
  }
  return parseEntityFromText(fallbackText);
}

function imageCapableEntity(type: string): boolean {
  return IMAGE_CAPABLE_TYPES.has(type);
}

function deriveImageReminders(cards: StreamCard[], currentDate: string): TaskDockItem[] {
  const now = new Date().toISOString();
  const reminders = new Map<string, TaskDockItem>();

  for (const card of cards) {
    const nodes = [
      ...(card.causal?.leftNodes ?? []),
      ...(card.causal?.rightNodes ?? []),
    ];
    for (const node of nodes) {
      const resolved = resolveEntityReminderPayload(node.entity, node.text);
      if (!resolved) {
        continue;
      }
      const normalizedType = normalizeEntityType(resolved.type);
      if (!imageCapableEntity(normalizedType)) {
        continue;
      }

      const missingImage =
        !node.image ||
        node.image.status === "missing" ||
        node.image.status === "upload_prompt";
      if (!missingImage) {
        continue;
      }

      const title = resolved.name.replace(/\s+/g, " ").trim();
      if (!title) {
        continue;
      }

      const link = resolved.link || `[[Entities/${typeFolderForReminder(normalizedType)}/${title}]]`;
      const key = `image-missing:${normalizedType}:${title.toLowerCase()}`;
      if (reminders.has(key)) {
        continue;
      }
      reminders.set(key, {
        id: crypto.randomUUID(),
        key,
        text: `Add image for ${link}`,
        kind: "reminder",
        completed: false,
        createdAt: now,
        updatedAt: now,
        targetDate: currentDate,
        sourceCardId: card.id,
        sourceNodeId: node.id,
      });
    }
  }

  return Array.from(reminders.values());
}

export function useLifeStreamContext() {
  const context = useContext(LifeStreamContext);
  if (!context) {
    throw new Error("useLifeStreamContext must be used within a LifeStreamProvider");
  }
  return context;
}

export function useLifeStreamContextOptional() {
  return useContext(LifeStreamContext);
}
