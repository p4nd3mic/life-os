import { useCallback, useEffect, useMemo, useState, useSyncExternalStore } from "react";
import { streamStore } from "../../state/streamStore";
import type { ClarificationOption } from "../../types";
import { CardImage } from "./CardImage";
import { ExpandedCard } from "./ExpandedCard";
import { ProcessingIndicator } from "./ProcessingIndicator";

const timeFormatter = new Intl.DateTimeFormat(undefined, {
  hour: "numeric",
  minute: "2-digit",
});

function formatTime(iso: string) {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) {
    return "";
  }
  return timeFormatter.format(date);
}

const GENERIC_TITLES = new Set([
  "response",
  "assistant response",
  "output",
  "result",
  "details",
]);

function sentenceTitle(value: string) {
  return value
    .trim()
    .replace(/\s+/g, " ")
    .split(" ")
    .map((part) => {
      if (!part) return part;
      return `${part.charAt(0).toUpperCase()}${part.slice(1).toLowerCase()}`;
    })
    .join(" ");
}

function displayTitleForCard(card: {
  title: string;
  originalInput?: string;
  emoji?: string;
}) {
  const normalized = card.title.trim().toLowerCase();
  const shouldFallback = GENERIC_TITLES.has(normalized);
  if (!shouldFallback || !card.originalInput?.trim()) {
    return card.title;
  }
  const prefix = card.emoji ? `${card.emoji} ` : "";
  return `${prefix}${sentenceTitle(card.originalInput)}`;
}

function formatDoneDuration(durationMs?: number) {
  if (!durationMs || durationMs <= 0) {
    return null;
  }
  const totalSeconds = Math.max(1, Math.round(durationMs / 1000));
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `Done in ${minutes}:${String(seconds).padStart(2, "0")}`;
}

type CardItemProps = {
  cardId: string;
  onCancel: (cardId: string) => void;
  onRetry: (cardId: string) => void;
  onClarify: (cardId: string, optionId: string) => void;
};

export function CardItem({ cardId, onCancel, onRetry, onClarify }: CardItemProps) {
  const card = useSyncExternalStore(
    (listener) => streamStore.subscribeToCard(cardId, listener),
    () => streamStore.getCard(cardId),
    () => undefined,
  );
  const [isExpanded, setIsExpanded] = useState(false);

  useEffect(() => {
    if (!card?.expanded || card.state !== "complete") {
      setIsExpanded(false);
    }
  }, [card?.expanded, card?.state]);

  const timeLabel = useMemo(
    () => (card ? formatTime(card.occurredAt) : ""),
    [card?.occurredAt],
  );
  const canCancel =
    card?.state === "pending" ||
    card?.state === "processing" ||
    card?.state === "awaiting_input";
  const canRetry = card?.state === "error";
  const canExpand = card?.state === "complete" && Boolean(card?.expanded);
  const clarificationOptions = card?.clarificationOptions ?? [];
  const displayTitle = useMemo(
    () => (card ? displayTitleForCard(card) : ""),
    [card],
  );
  const doneDurationLabel = useMemo(
    () => (card?.state === "complete" ? formatDoneDuration(card.durationMs) : null),
    [card?.durationMs, card?.state],
  );

  const toggleExpanded = useCallback(() => {
    if (!canExpand) return;
    setIsExpanded((prev) => !prev);
  }, [canExpand]);

  const handleClarify = useCallback((option: ClarificationOption) => {
    if (!card) return;
    onClarify(card.id, option.id);
  }, [card, onClarify]);

  if (!card) return null;

  return (
    <article
      className={`life-card life-stream-card state-${card.state}${isExpanded ? " is-expanded" : ""}`}
      aria-expanded={isExpanded}
    >
      <header className="life-stream-card__header">
        <div className="life-stream-card__emoji" aria-hidden>
          {card.emoji}
        </div>
        <div className="life-stream-card__meta">
          <div className="life-stream-card__title">{displayTitle}</div>
          {card.subtitle && (
            <div className="life-stream-card__subtitle">{card.subtitle}</div>
          )}
          {!card.subtitle && doneDurationLabel && (
            <div className="life-stream-card__subtitle">{doneDurationLabel}</div>
          )}
        </div>
        <div className="life-stream-card__time">{timeLabel}</div>
        {canExpand && (
          <button
            type="button"
            className="life-stream-card__expand"
            onClick={toggleExpanded}
            aria-label={isExpanded ? "Collapse card" : "Expand card"}
            aria-expanded={isExpanded}
          >
            {isExpanded ? "▼" : "▶"}
          </button>
        )}
      </header>

      {card.image && (
        <CardImage
          image={card.image}
          title={card.title}
          emoji={card.emoji}
          size={isExpanded ? "expanded" : "compact"}
        />
      )}

      {card.summary && (
        <div className="life-stream-card__summary">{card.summary}</div>
      )}

      {card.stats && (
        <div className="life-stream-card__stats">
          {Object.entries(card.stats).map(([key, value]) => (
            <div key={key} className="life-stream-card__stat">
              <span className="life-stream-card__stat-label">{key}</span>
              <span className="life-stream-card__stat-value">{String(value)}</span>
            </div>
          ))}
        </div>
      )}

      {card.entities && card.entities.length > 0 && (
        <div className="life-stream-card__entities">
          {card.entities.map((entity) => (
            <span key={`${entity.type}-${entity.name}`} className="life-stream-card__entity">
              {entity.name}
            </span>
          ))}
        </div>
      )}

      {card.state === "awaiting_input" && clarificationOptions.length > 0 && (
        <div className="life-stream-card__clarification">
          <div className="life-stream-card__clarification-message">
            {card.processingStep ?? "Need more info"}
          </div>
          <div className="life-stream-card__clarification-options">
            {clarificationOptions.map((option) => (
              <button
                key={option.id}
                type="button"
                className="life-stream-card__clarification-option"
                onClick={() => handleClarify(option)}
              >
                {option.emoji && <span aria-hidden>{option.emoji}</span>}
                {option.label}
              </button>
            ))}
          </div>
        </div>
      )}

      {card.errorMessage && (
        <div role="alert" className="life-stream-card__error">
          {card.errorMessage}
        </div>
      )}

      {(canCancel || canRetry) && (
        <div className="life-stream-card__actions">
          {canCancel && (
            <button
              type="button"
              className="life-stream-card__action"
              onClick={() => onCancel(card.id)}
            >
              Cancel
            </button>
          )}
          {canRetry && (
            <button
              type="button"
              className="life-stream-card__action is-primary"
              onClick={() => onRetry(card.id)}
            >
              Retry
            </button>
          )}
        </div>
      )}

      <ProcessingIndicator card={card} />

      {isExpanded && canExpand && (
        <ExpandedCard card={card} onCollapse={() => setIsExpanded(false)} />
      )}
    </article>
  );
}
