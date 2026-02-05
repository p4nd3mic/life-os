import { useMemo, useSyncExternalStore } from "react";
import { streamStore } from "../state/streamStore";
import { CardBubble } from "./stream/CardBubble";
import { formatPacificTimeLabel } from "../../../utils/pacificTime";
import "./LifeMessageRow.css";

type LifeMessageRowProps = {
  cardId: string;
  index: number;
  gapPx?: number;
  onCancel: (cardId: string) => void;
  onRetry: (cardId: string) => void;
  onClarify: (cardId: string, optionId: string) => void;
};

export function LifeMessageRow({
  cardId,
  index,
  gapPx,
  onCancel,
  onRetry,
  onClarify,
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

  if (!card) {
    return null;
  }

  const sideClass = index % 2 === 0 ? "is-left" : "is-right";

  return (
    <div
      className={`life-message-row ${sideClass}`}
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
        />
      </div>
    </div>
  );
}
