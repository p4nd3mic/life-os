import type { StreamCard, StreamLayoutMode } from "../types";

export const DEFAULT_STREAM_LAYOUT_MODE: StreamLayoutMode = "cause_effect";

export function resolvePersistedLayoutMode(
  card: Pick<StreamCard, "layoutMode" | "causal">,
): StreamLayoutMode {
  if (card.layoutMode) {
    return card.layoutMode;
  }
  if (card.causal) {
    return DEFAULT_STREAM_LAYOUT_MODE;
  }
  return "classic";
}

export function resolveRenderableLayoutMode(
  card: Pick<StreamCard, "layoutMode" | "causal">,
): StreamLayoutMode {
  const persisted = resolvePersistedLayoutMode(card);
  if (persisted === "cause_effect" && !card.causal) {
    // Phase 0 compatibility: until the dedicated cause/effect renderer lands,
    // fall back to classic rendering when structured graph payload is absent.
    return "classic";
  }
  return persisted;
}
