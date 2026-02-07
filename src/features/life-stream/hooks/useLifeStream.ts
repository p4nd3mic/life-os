import { useCallback, useEffect, useRef, useState, useSyncExternalStore } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { streamStore } from "../state/streamStore";
import type {
  CausalRestructureAction,
  CausalRestructureResult,
  DayThreadDebugSummary,
  LifeStreamAuthHealth,
  ImageAutoFetchMode,
  ImageAutoFetchSummary,
  ImageAttachResult,
  ImageCandidateResponse,
  LifeStreamEvent,
  SemanticRegenerationResult,
  StreamCard,
} from "../types";

type SubmitStatus = {
  state: "idle" | "sending" | "received" | "created" | "error";
  message?: string;
};

type SubmitOptions = {
  occurredAtIso?: string;
  modelId?: string | null;
  effort?: string | null;
  accessMode?: "read-only" | "current" | "full-access";
  collaborationMode?: Record<string, unknown> | null;
};

export function useLifeStream(workspaceId: string | null) {
  const [isLoading, setIsLoading] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [submitStatus, setSubmitStatus] = useState<SubmitStatus>({ state: "idle" });
  const requestIdRef = useRef(0);
  const submitTimerRef = useRef<number | null>(null);

  const clearSubmitStatus = useCallback(() => {
    setSubmitStatus({ state: "idle" });
  }, []);

  const scheduleSubmitClear = useCallback((delayMs = 2500) => {
    if (submitTimerRef.current) {
      window.clearTimeout(submitTimerRef.current);
    }
    submitTimerRef.current = window.setTimeout(() => {
      setSubmitStatus({ state: "idle" });
      submitTimerRef.current = null;
    }, delayMs);
  }, []);

  // Subscribe to card list changes
  const cards = useSyncExternalStore(
    (callback) => streamStore.subscribe(callback),
    () => streamStore.getSnapshot(),
    () => [],
  );

  const currentDate = streamStore.getCurrentDate();

  // Load cards for current date
  const loadDay = useCallback(async (dateIso: string) => {
    if (!workspaceId) return;

    streamStore.setDate(dateIso);
    const requestId = ++requestIdRef.current;
    setIsLoading(true);
    setLoadError(null);

    try {
      const cards = await invoke<StreamCard[]>("life_stream_load_day", {
        workspaceId,
        dateIso,
      });
      if (requestIdRef.current === requestId) {
        streamStore.loadCards(cards);
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      console.error("Failed to load day:", message);
      if (requestIdRef.current === requestId) {
        setLoadError(message);
      }
    } finally {
      if (requestIdRef.current === requestId) {
        setIsLoading(false);
      }
    }
  }, [workspaceId]);

  // Submit new input
  const submit = useCallback(async (input: string, options?: SubmitOptions) => {
    if (!workspaceId) return;

    const cardId = crypto.randomUUID();
    const now = new Date().toISOString();
    setSubmitStatus({ state: "sending", message: "Sending..." });

    // Optimistic: add pending card immediately
    const pendingCard: StreamCard = {
      id: cardId,
      occurredAt: options?.occurredAtIso ?? now,
      createdAt: now,
      updatedAt: now,
      version: 1,
      cardType: "generic",
      domain: "general",
      emoji: "📝",
      layoutMode: "cause_effect",
      state: "pending",
      processingStep: "Submitting...",
      processingSteps: ["Submitting..."],
      title: input.slice(0, 50) + (input.length > 50 ? "..." : ""),
      originalInput: input,
      request: options
        ? {
            model: options.modelId ?? undefined,
            effort: options.effort ?? undefined,
            accessMode: options.accessMode ?? undefined,
          }
        : undefined,
    };

    streamStore.addCard(pendingCard);

    try {
      await invoke("life_stream_submit", {
        workspaceId,
        cardId,
        input,
        occurredAtIso: options?.occurredAtIso,
        modelId: options?.modelId ?? null,
        effort: options?.effort ?? null,
        accessMode: options?.accessMode ?? null,
        collaborationMode: options?.collaborationMode ?? null,
      });
      setSubmitStatus({ state: "received", message: "Message received" });
      scheduleSubmitClear();
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      streamStore.setCardError(cardId, message, 2);
      setSubmitStatus({ state: "error", message });
    }
  }, [workspaceId]);

  const cancel = useCallback(async (cardId: string) => {
    if (!workspaceId) return;
    const existing = streamStore.getCard(cardId);
    if (existing) {
      streamStore.updateCard(
        cardId,
        { state: "cancelled", processingStep: "Cancelled" },
        existing.version + 1,
      );
    }
    try {
      await invoke("life_stream_cancel", { workspaceId, cardId });
    } catch (err) {
      console.error("Failed to cancel card:", err);
    }
  }, [workspaceId]);

  const retry = useCallback(async (cardId: string) => {
    if (!workspaceId) return;
    const existing = streamStore.getCard(cardId);
    if (existing) {
      streamStore.updateCard(
        cardId,
        { state: "processing", processingStep: "Retrying...", errorMessage: undefined },
        existing.version + 1,
      );
    }
    try {
      await invoke("life_stream_retry", { workspaceId, cardId });
    } catch (err) {
      console.error("Failed to retry card:", err);
      if (existing) {
        streamStore.setCardError(cardId, String(err), existing.version + 2);
      }
    }
  }, [workspaceId]);

  const clarify = useCallback(async (cardId: string, optionId: string) => {
    if (!workspaceId) return;
    const existing = streamStore.getCard(cardId);
    if (existing) {
      streamStore.updateCard(
        cardId,
        {
          state: "processing",
          processingStep: "Resuming...",
          clarificationOptions: [],
        },
        existing.version + 1,
      );
    }
    try {
      await invoke("life_stream_clarify", { workspaceId, cardId, optionId });
    } catch (err) {
      console.error("Failed to clarify card:", err);
    }
  }, [workspaceId]);

  const restructure = useCallback(
    async (
      cardId: string,
      action: CausalRestructureAction,
      options?: {
        sourceNodeIds?: string[];
        targetMode?: "cause_effect" | "action_reward";
      },
    ) => {
      if (!workspaceId) return;

      try {
        const result = await invoke<CausalRestructureResult>("life_stream_restructure", {
          workspaceId,
          cardId,
          action,
          sourceNodeIds: options?.sourceNodeIds ?? null,
          targetMode: options?.targetMode ?? null,
        });
        streamStore.updateCard(cardId, result.patch, result.version);
      } catch (err) {
        console.error("Failed to restructure card:", err);
      }
    },
    [workspaceId],
  );

  const regenerateSemantics = useCallback(
    async (
      cardIds: string[],
      options?: { forceLlm?: boolean; persist?: boolean },
    ): Promise<SemanticRegenerationResult | null> => {
      if (!workspaceId || cardIds.length === 0) {
        return null;
      }
      try {
        return await invoke<SemanticRegenerationResult>("life_stream_regenerate_semantics", {
          workspaceId,
          cardIds,
          forceLlm: options?.forceLlm ?? true,
          persist: options?.persist ?? true,
        });
      } catch (err) {
        console.error("Failed to regenerate semantics:", err);
        return null;
      }
    },
    [workspaceId],
  );

  const getDayThreadDebug = useCallback(
    async (dateIso: string): Promise<DayThreadDebugSummary | null> => {
      if (!workspaceId) {
        return null;
      }
      try {
        return await invoke<DayThreadDebugSummary>("life_stream_day_thread_debug", {
          workspaceId,
          dateIso,
        });
      } catch (err) {
        console.error("Failed to load day-thread debug summary:", err);
        return null;
      }
    },
    [workspaceId],
  );

  const getAuthHealth = useCallback(async (): Promise<LifeStreamAuthHealth | null> => {
    if (!workspaceId) {
      return null;
    }
    try {
      return await invoke<LifeStreamAuthHealth>("life_stream_auth_health", {
        workspaceId,
      });
    } catch (err) {
      console.error("Failed to check Life Stream auth health:", err);
      return null;
    }
  }, [workspaceId]);

  const resetDayThreadAndRebuild = useCallback(
    async (
      dateIso: string,
      persist = true,
    ): Promise<SemanticRegenerationResult | null> => {
      if (!workspaceId) {
        return null;
      }
      try {
        return await invoke<SemanticRegenerationResult>(
          "life_stream_day_thread_reset_and_rebuild",
          {
            workspaceId,
            dateIso,
            persist,
          },
        );
      } catch (err) {
        console.error("Failed to reset day-thread and rebuild semantics:", err);
        return null;
      }
    },
    [workspaceId],
  );

  const getImageCandidates = useCallback(
    async (cardId: string, nodeId?: string | null): Promise<ImageCandidateResponse | null> => {
      if (!workspaceId) return null;
      try {
        return await invoke<ImageCandidateResponse>("life_stream_image_candidates", {
          workspaceId,
          cardId,
          nodeId: nodeId ?? null,
        });
      } catch (err) {
        console.error("Failed to load image candidates:", err);
        return null;
      }
    },
    [workspaceId],
  );

  const attachImage = useCallback(
    async (
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
    ): Promise<ImageAttachResult | null> => {
      if (!workspaceId) return null;
      try {
        const result = await invoke<ImageAttachResult>("life_stream_image_attach", {
          workspaceId,
          cardId,
          nodeId: options?.nodeId ?? null,
          sourcePath,
          setPrimary: options?.setPrimary ?? true,
          setContextOverride: options?.setContextOverride ?? false,
          contextHint: options?.contextHint ?? null,
          updateEntityFile: options?.updateEntityFile ?? true,
          updateEntityEmbed: options?.updateEntityEmbed ?? false,
        });
        streamStore.updateCard(cardId, result.patch, result.version);
        return result;
      } catch (err) {
        console.error("Failed to attach image:", err);
        return null;
      }
    },
    [workspaceId],
  );

  const autoFetchImages = useCallback(
    async (
      cardIds: string[],
      options?: {
        mode?: ImageAutoFetchMode;
        updateEntityFile?: boolean;
        updateEntityEmbed?: boolean;
      },
    ): Promise<ImageAutoFetchSummary | null> => {
      if (!workspaceId || cardIds.length === 0) {
        return null;
      }
      try {
        return await invoke<ImageAutoFetchSummary>("life_stream_image_autofetch", {
          workspaceId,
          cardIds,
          mode: options?.mode ?? "review_first",
          updateEntityFile: options?.updateEntityFile ?? true,
          updateEntityEmbed: options?.updateEntityEmbed ?? false,
        });
      } catch (err) {
        console.error("Failed to auto-fetch images:", err);
        return null;
      }
    },
    [workspaceId],
  );

  // Navigate to previous/next day
  const goToPreviousDay = useCallback(() => {
    const date = new Date(currentDate);
    date.setDate(date.getDate() - 1);
    void loadDay(date.toISOString().split("T")[0]);
  }, [currentDate, loadDay]);

  const goToNextDay = useCallback(() => {
    const date = new Date(currentDate);
    date.setDate(date.getDate() + 1);
    void loadDay(date.toISOString().split("T")[0]);
  }, [currentDate, loadDay]);

  const goToToday = useCallback(() => {
    void loadDay(new Date().toISOString().split("T")[0]);
  }, [loadDay]);

  // Listen for stream events
  useEffect(() => {
    if (!workspaceId) return;

    const unlisten = listen<LifeStreamEvent>("life_stream_event", (event) => {
      streamStore.handleEvent(event.payload);
    });

    return () => {
      void unlisten.then((fn) => fn());
    };
  }, [workspaceId]);

  // Load today on mount
  useEffect(() => {
    if (workspaceId) {
      void loadDay(new Date().toISOString().split("T")[0]);
    }
  }, [workspaceId, loadDay]);

  return {
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
    loadDay,
    goToPreviousDay,
    goToNextDay,
    goToToday,
  };
}
