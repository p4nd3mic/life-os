import { useMemo, useSyncExternalStore } from "react";
import { streamStore } from "../state/streamStore";
import { CardBubble } from "./stream/CardBubble";
import { formatPacificTimeLabel } from "../../../utils/pacificTime";
import { cardAnchorId } from "../utils/anchors";
import type { CausalRestructureAction } from "../types";
import "./LifeMessageRow.css";

type LifeMessageRowProps = {
  cardId: string;
  gapPx?: number;
  onCancel: (cardId: string) => void;
  onRetry: (cardId: string) => void;
  onClarify: (cardId: string, optionId: string) => void;
  onRestructure: (
    cardId: string,
    action: CausalRestructureAction,
    options?: { sourceNodeIds?: string[]; targetMode?: "cause_effect" | "action_reward" },
  ) => void;
};

export function LifeMessageRow({
  cardId,
  gapPx,
  onCancel,
  onRetry,
  onClarify,
  onRestructure,
}: LifeMessageRowProps) {
  const card = useSyncExternalStore(
    (listener) => streamStore.subscribeToCard(cardId, listener),
    () => streamStore.getCard(cardId),
    () => undefined,
  );

  const timeLabel = useMemo(() => {
    if (!card) return "";
    return formatPacificTimeLabel(card.occurredAt);
  }, [card]);

  const isCauseEffect = Boolean(
    card?.layoutMode === "cause_effect" && card?.causal,
  );

  if (!card) {
    return null;
  }

  return (
    <div
      className={`life-message-row${isCauseEffect ? " life-message-row--cause-effect" : ""}`}
      id={cardAnchorId(cardId)}
      style={gapPx ? { marginTop: `${gapPx}px` } : undefined}
    >
      <div className="life-message-row__time" aria-hidden="true">
        {timeLabel}
      </div>
      <div className="life-message-row__dot" aria-hidden="true" />
      <div className="life-message-row__card">
        <CardBubble
          card={card}
          onCancel={onCancel}
          onRetry={onRetry}
          onClarify={onClarify}
          onRestructure={onRestructure}
        />
      </div>
    </div>
  );
}
