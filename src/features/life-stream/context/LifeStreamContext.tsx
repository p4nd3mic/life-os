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
  DomainId,
  EntityRef,
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
  };
  addTaskDockItem: (text: string) => void;
  toggleTaskDockItem: (id: string) => void;
  moveTaskDockItem: (id: string, direction: -1 | 1, visibleIds: string[]) => void;
  setTaskDockFilter: (filter: TaskDockFilter) => void;
  setTaskDockCollapsed: (collapsed: boolean) => void;
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
  const [imageAutoFetchStatus, setImageAutoFetchStatus] = useState<{
    state: "idle" | "running" | "done" | "error";
    message?: string;
  }>({ state: "idle" });
  const semanticStatusTimerRef = useRef<number | null>(null);
  const imageAutoFetchStatusTimerRef = useRef<number | null>(null);

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
      scheduleSemanticStatusClear(4200);
      return null;
    }

    if (result.failed > 0 && result.updated === 0) {
      const firstError = result.errors[0];
      const message = firstError
        ? `⚠️ Rebuild failed (0 updated, ${result.failed} failed): ${firstError}`
        : `⚠️ Rebuild failed (0 updated, ${result.failed} failed).`;
      setSemanticRegenerationStatus({ state: "error", message });
      scheduleSemanticStatusClear(5200);
      return result;
    }

    if (result.failed > 0) {
      const message = `🟡 Rebuilt ${result.updated} · failed ${result.failed}`;
      setSemanticRegenerationStatus({ state: "done", message });
      scheduleSemanticStatusClear(4200);
      return result;
    }

    const message = `✅ Rebuilt ${result.updated}`;
    setSemanticRegenerationStatus({ state: "done", message });
    scheduleSemanticStatusClear(3200);
    return result;
  }, [cards, regenerateSemantics, scheduleSemanticStatusClear, workspaceId]);

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
  };

  return (
    <LifeStreamContext.Provider value={value}>
      {children}
    </LifeStreamContext.Provider>
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
