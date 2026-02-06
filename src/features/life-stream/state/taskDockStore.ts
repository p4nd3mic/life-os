import type {
  TaskDockFilter,
  TaskDockItem,
  TaskDockItemKind,
  TaskDockPayload,
} from "../types";

type Listener = () => void;

type TaskDockSnapshot = {
  items: TaskDockItem[];
  filter: TaskDockFilter;
  collapsed: boolean;
};

const PREF_STORAGE_KEY = "life-stream-task-dock-prefs.v1";

function defaultSnapshot(): TaskDockSnapshot {
  return {
    items: [],
    filter: "today",
    collapsed: false,
  };
}

function sanitizeText(value: string): string {
  return value.trim().replace(/\s+/g, " ");
}

class TaskDockStore {
  private listeners: Set<Listener> = new Set();
  private snapshot: TaskDockSnapshot = defaultSnapshot();

  constructor() {
    this.snapshot = this.loadPrefs(defaultSnapshot());
  }

  subscribe(listener: Listener): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  getSnapshot(): TaskDockSnapshot {
    return this.snapshot;
  }

  getPayload(): TaskDockPayload {
    return {
      version: 1,
      items: this.snapshot.items,
    };
  }

  hydrate(payload: TaskDockPayload): void {
    const items = Array.isArray(payload.items) ? payload.items : [];
    this.snapshot = {
      ...this.snapshot,
      items: items
        .filter((item) => Boolean(item?.id && item?.text && item?.key))
        .map((item) => ({
          ...item,
          text: sanitizeText(item.text),
          key: item.key.trim(),
        })),
    };
    this.notify();
  }

  setFilter(filter: TaskDockFilter): void {
    if (this.snapshot.filter === filter) return;
    this.snapshot = {
      ...this.snapshot,
      filter,
    };
    this.persistPrefs();
    this.notify();
  }

  setCollapsed(collapsed: boolean): void {
    if (this.snapshot.collapsed === collapsed) return;
    this.snapshot = {
      ...this.snapshot,
      collapsed,
    };
    this.persistPrefs();
    this.notify();
  }

  addManualTask(text: string, targetDate: string): void {
    const normalized = sanitizeText(text);
    if (!normalized) return;
    const now = new Date().toISOString();
    const id = crypto.randomUUID();
    const item: TaskDockItem = {
      id,
      key: `manual:${id}`,
      text: normalized,
      kind: "task",
      completed: false,
      createdAt: now,
      updatedAt: now,
      targetDate,
    };

    this.snapshot = {
      ...this.snapshot,
      items: [item, ...this.snapshot.items],
    };
    this.notify();
  }

  toggleComplete(id: string): void {
    const now = new Date().toISOString();
    let changed = false;
    const items = this.snapshot.items.map((item) => {
      if (item.id !== id) return item;
      changed = true;
      return {
        ...item,
        completed: !item.completed,
        updatedAt: now,
      };
    });

    if (!changed) return;
    this.snapshot = { ...this.snapshot, items };
    this.notify();
  }

  moveByVisibleIds(id: string, direction: -1 | 1, visibleIds: string[]): void {
    const currentVisibleIndex = visibleIds.indexOf(id);
    if (currentVisibleIndex < 0) return;

    const targetVisibleIndex = currentVisibleIndex + direction;
    if (targetVisibleIndex < 0 || targetVisibleIndex >= visibleIds.length) return;

    const targetId = visibleIds[targetVisibleIndex];
    const fromIndex = this.snapshot.items.findIndex((item) => item.id === id);
    const toIndex = this.snapshot.items.findIndex((item) => item.id === targetId);
    if (fromIndex < 0 || toIndex < 0) return;

    const items = [...this.snapshot.items];
    const [moved] = items.splice(fromIndex, 1);
    items.splice(toIndex, 0, moved);
    this.snapshot = { ...this.snapshot, items };
    this.notify();
  }

  upsertInferredReminders(items: TaskDockItem[]): void {
    if (!items.length) return;
    const existingKeys = new Set(this.snapshot.items.map((item) => item.key));
    const next = [...this.snapshot.items];
    let changed = false;

    for (const item of items) {
      if (existingKeys.has(item.key)) {
        continue;
      }
      existingKeys.add(item.key);
      next.unshift({
        ...item,
        text: sanitizeText(item.text),
      });
      changed = true;
    }

    if (!changed) return;
    this.snapshot = { ...this.snapshot, items: next };
    this.notify();
  }

  private loadPrefs(base: TaskDockSnapshot): TaskDockSnapshot {
    if (typeof window === "undefined") {
      return base;
    }

    try {
      const raw = window.localStorage.getItem(PREF_STORAGE_KEY);
      if (!raw) return base;
      const parsed = JSON.parse(raw) as Partial<TaskDockSnapshot>;
      return {
        ...base,
        filter: parsed.filter === "all" ? "all" : "today",
        collapsed: Boolean(parsed.collapsed),
      };
    } catch {
      return base;
    }
  }

  private persistPrefs(): void {
    if (typeof window === "undefined") return;
    try {
      window.localStorage.setItem(
        PREF_STORAGE_KEY,
        JSON.stringify({
          filter: this.snapshot.filter,
          collapsed: this.snapshot.collapsed,
        }),
      );
    } catch {
      // ignore storage failures
    }
  }

  private notify(): void {
    for (const listener of this.listeners) {
      listener();
    }
  }
}

export const taskDockStore = new TaskDockStore();
export type { TaskDockSnapshot, TaskDockItemKind };

