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
  ImageAttachResult,
  ImageCandidateResponse,
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
    getImageCandidates,
    attachImage,
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
