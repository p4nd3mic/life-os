import {
  createContext,
  useCallback,
  useContext,
  useState,
  type ReactNode,
} from "react";
import { useLifeStream } from "../hooks/useLifeStream";
import type { DomainId, StreamCard } from "../types";

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
    goToPreviousDay,
    goToNextDay,
    goToToday,
  } = useLifeStream(workspaceId);

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
  };

  return (
    <LifeStreamContext.Provider value={value}>
      {children}
    </LifeStreamContext.Provider>
  );
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
