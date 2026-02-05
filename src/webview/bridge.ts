import type { StreamCard } from "../features/life-stream/types";
import type { WorkspaceInfo } from "../types";

type BridgeRequest = {
  id: string;
  method: string;
  payload?: Record<string, unknown> | null;
};

type BridgeResponse = {
  id: string;
  ok: boolean;
  result?: unknown;
  error?: string;
};

export type CodexBridge = {
  invoke: (method: string, payload?: Record<string, unknown> | null) => Promise<unknown>;
  onNativeMessage: (message: BridgeResponse | string) => void;
  listen: (eventName: string, handler: (detail: unknown) => void) => () => void;
  workspaceId?: string | null;
};

declare global {
  interface Window {
    __codexBridge__?: CodexBridge;
    __WEBVIEW_DEBUG__?: boolean;
    webkit?: {
      messageHandlers?: {
        codexBridge?: { postMessage: (payload: BridgeRequest) => void };
      };
    };
  }
}

const pending = new Map<
  string,
  { resolve: (value: unknown) => void; reject: (error: Error) => void }
>();

const DEBUG_WORKSPACE: WorkspaceInfo = {
  id: "life-webview",
  name: "Life OS",
  path: "/Users/jmwillis/LifeOS",
  connected: true,
  settings: {
    sidebarCollapsed: true,
    purpose: "life",
  },
};

const DEBUG_MODELS = [
  {
    id: "gpt-5.3-codex",
    model: "gpt-5.3-codex",
    displayName: "GPT-5.3 Codex",
    description: "Default Codex model",
    supportedReasoningEfforts: [
      { reasoningEffort: "low", description: "Quick" },
      { reasoningEffort: "medium", description: "Balanced" },
      { reasoningEffort: "high", description: "Deep" },
    ],
    defaultReasoningEffort: "medium",
    isDefault: true,
  },
];

function isDebugMode() {
  return Boolean(window.__WEBVIEW_DEBUG__ || import.meta.env.VITE_WEBVIEW_DEBUG);
}

function buildDebugCards(): StreamCard[] {
  const now = new Date();
  const makeTime = (minutesAgo: number) =>
    new Date(now.getTime() - minutesAgo * 60_000).toISOString();
  return [
    {
      id: "preview-episode-5",
      occurredAt: makeTime(120),
      createdAt: makeTime(120),
      updatedAt: makeTime(118),
      version: 3,
      cardType: "thought",
      domain: "general",
      emoji: "📝",
      state: "complete",
      title: "Episode 5 is the real start",
      summary:
        "Ep 5 reframes Spike’s detachment and turns Vicious into a true gravitational pull.",
      originalInput:
        "I think episode 5 of Cowboy Bebop is my favorite — it feels like the real show starts here.",
      expanded: {
        sections: [
          {
            title: "Codex response",
            body:
              "Episode 5 is where the series flips from vibe to destiny — Spike stops drifting and starts spiraling toward Vicious.",
          },
        ],
        actions: [],
      },
    },
    {
      id: "preview-morning-log",
      occurredAt: makeTime(420),
      createdAt: makeTime(420),
      updatedAt: makeTime(418),
      version: 2,
      cardType: "generic",
      domain: "general",
      emoji: "📝",
      state: "complete",
      title: "Running a test",
      summary: "Codex is responding. ✅ Online and receiving your messages.",
      originalInput: "running a test",
      stats: {
        status: "online",
      },
    },
    {
      id: "preview-delivery",
      occurredAt: makeTime(520),
      createdAt: makeTime(520),
      updatedAt: makeTime(520),
      version: 1,
      cardType: "delivery_order",
      domain: "delivery",
      emoji: "🚗",
      state: "complete",
      title: "Dinner shift started",
      summary: "5:58pm — Started dinner shift. AR at 78%.",
      originalInput: "Started dinner shift from Riviera Village.",
    },
  ];
}

async function debugInvoke(method: string, payload?: Record<string, unknown> | null) {
  switch (method) {
    case "list_workspaces":
      return [DEBUG_WORKSPACE];
    case "connect_workspace":
      return null;
    case "model_list":
      return { data: DEBUG_MODELS };
    case "collaboration_mode_list":
      return { data: [] };
    case "skills_list":
      return { data: [], skills: [] };
    case "prompts_list":
      return { prompts: [] };
    case "list_workspace_files":
      return [];
    case "life_stream_load_day":
      return buildDebugCards();
    case "life_stream_read_log":
      return [
        "Life Stream booted in debug mode.",
        `workspace=${(payload?.workspaceId as string | undefined) ?? "unknown"}`,
      ];
    case "life_stream_submit":
    case "life_stream_cancel":
    case "life_stream_retry":
    case "life_stream_clarify":
      return null;
    default:
      return null;
  }
}

function hasNativeBridge() {
  return Boolean(window.webkit?.messageHandlers?.codexBridge?.postMessage);
}

export function initCodexBridge(): CodexBridge {
  if (window.__codexBridge__) {
    return window.__codexBridge__!;
  }

  const bridge: CodexBridge = {
    invoke: async (method, payload = null) => {
      if (!hasNativeBridge()) {
        if (isDebugMode()) {
          return debugInvoke(method, payload);
        }
        throw new Error("Codex bridge unavailable.");
      }

      const id = crypto.randomUUID();
      return new Promise((resolve, reject) => {
        pending.set(id, { resolve, reject });
        try {
          window.webkit!.messageHandlers!.codexBridge!.postMessage({
            id,
            method,
            payload,
          });
        } catch (error) {
          pending.delete(id);
          reject(error instanceof Error ? error : new Error(String(error)));
        }
      });
    },
    onNativeMessage: (message) => {
      let parsed: BridgeResponse | null = null;
      if (typeof message === "string") {
        try {
          parsed = JSON.parse(message) as BridgeResponse;
        } catch {
          parsed = null;
        }
      } else {
        parsed = message;
      }

      if (!parsed || !parsed.id) {
        return;
      }

      const entry = pending.get(parsed.id);
      if (!entry) {
        return;
      }
      pending.delete(parsed.id);

      if (parsed.ok) {
        entry.resolve(parsed.result);
      } else {
        entry.reject(new Error(parsed.error ?? "Bridge error"));
      }
    },
    listen: (eventName, handler) => {
      const listener = (event: Event) => {
        handler((event as CustomEvent).detail);
      };
      window.addEventListener(eventName, listener as EventListener);
      return () => window.removeEventListener(eventName, listener as EventListener);
    },
    workspaceId: null,
  };

  window.__codexBridge__ = bridge;
  return bridge;
}

export function getCodexBridge(): CodexBridge | null {
  return window.__codexBridge__ ?? null;
}

export function setBridgeWorkspaceId(id: string | null) {
  const bridge = initCodexBridge();
  bridge.workspaceId = id;
}
