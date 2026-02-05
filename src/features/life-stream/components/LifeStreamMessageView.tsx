import { useEffect, useMemo, useState } from "react";
import { useLifeStreamContext } from "../context/LifeStreamContext";
import { CardErrorBoundary } from "./stream/CardErrorBoundary";
import { LifeMessageRow } from "./LifeMessageRow";
import { formatPacificTimeLabel, getPacificDateString } from "../../../utils/pacificTime";
import "./LifeStreamMessageView.css";

function parseOccurredAt(value: string): number | null {
  const date = new Date(value);
  if (!Number.isNaN(date.getTime())) {
    return date.getTime();
  }
  const fallback = new Date(`${value}Z`);
  if (!Number.isNaN(fallback.getTime())) {
    return fallback.getTime();
  }
  return null;
}

function LifeStreamNowMarker({
  label,
  gapPx = 0,
}: {
  label: string;
  gapPx?: number;
}) {
  return (
    <div
      className="life-stream-now-marker"
      aria-hidden="true"
      style={gapPx ? { marginTop: `${gapPx}px` } : undefined}
    >
      <div className="life-stream-now-marker__dot" />
      <div className="life-stream-now-marker__label">{label}</div>
    </div>
  );
}

function computeGapPx(gapMinutes: number): number {
  const gapPx = 12 + Math.log1p(gapMinutes) * 12;
  return Math.min(96, Math.max(12, Math.round(gapPx)));
}

type TimelineItemBase =
  | {
      kind: "card";
      id: string;
      cardIndex: number;
      timeMs: number | null;
    }
  | {
      kind: "now";
      label: string;
      timeMs: number | null;
    };

type TimelineItem =
  | {
      kind: "card";
      id: string;
      cardIndex: number;
      timeMs: number | null;
      gapPx: number;
      gapMinutes: number;
    }
  | {
      kind: "now";
      label: string;
      timeMs: number | null;
      gapPx: number;
      gapMinutes: number;
    };

export function LifeStreamMessageView() {
  const { filteredCards, isLoading, cancel, retry, clarify, loadError, currentDate } =
    useLifeStreamContext();

  const [now, setNow] = useState<Date>(() => new Date());
  const isToday = currentDate === getPacificDateString();

  useEffect(() => {
    if (!isToday) return;
    const id = window.setInterval(() => setNow(new Date()), 60_000);
    return () => window.clearInterval(id);
  }, [isToday]);

  const nowLabel = useMemo(() => {
    if (!isToday) return "";
    const time = formatPacificTimeLabel(now.toISOString());
    return time ? `Now · ${time}` : "Now";
  }, [isToday, now]);

  const timelineItems = useMemo<TimelineItem[]>(() => {
    if (filteredCards.length === 0) return [];
    const nowTime = isToday ? now.getTime() : null;
    const items: TimelineItemBase[] = [];

    let nowInserted = false;
    let nowIndex: number | null = null;
    if (isToday && nowTime !== null && !Number.isNaN(nowTime)) {
      for (let index = 0; index < filteredCards.length; index += 1) {
        const cardTime = parseOccurredAt(filteredCards[index].occurredAt);
        if (cardTime === null) continue;
        if (cardTime <= nowTime) {
          nowIndex = index;
          break;
        }
      }
      if (nowIndex === null) {
        nowIndex = filteredCards.length;
      }
    }

    filteredCards.forEach((card, index) => {
      if (isToday && nowLabel && nowIndex === index) {
        items.push({
          kind: "now",
          label: nowLabel,
          timeMs: nowTime,
        });
        nowInserted = true;
      }
      items.push({
        kind: "card",
        id: card.id,
        cardIndex: index,
        timeMs: parseOccurredAt(card.occurredAt),
      });
    });

    if (isToday && nowLabel && !nowInserted) {
      items.push({
        kind: "now",
        label: nowLabel,
        timeMs: nowTime,
      });
    }

    const enriched: TimelineItem[] = [];
    let prevTime: number | null = null;
    items.forEach((item, index) => {
      let gapMinutes = 0;
      if (index > 0 && prevTime !== null && item.timeMs !== null) {
        gapMinutes = Math.max(
          0,
          Math.round((prevTime - item.timeMs) / 60000),
        );
      }
      const gapPx = computeGapPx(gapMinutes);
      enriched.push({ ...item, gapPx, gapMinutes });
      if (item.timeMs !== null) {
        prevTime = item.timeMs;
      }
    });

    return enriched;
  }, [filteredCards, isToday, now, nowLabel]);

  const hasCards = filteredCards.length > 0;
  const containerClassName = [
    "messages",
    "life-stream-messages",
    hasCards ? "messages-full" : "",
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <div className={containerClassName}>
      {loadError && (
        <div className="life-stream-error-banner" role="status">
          {loadError}
        </div>
      )}
      {isLoading ? (
        <div className="life-stream-loading">Loading stream...</div>
      ) : hasCards ? (
        <section className="life-stream-message-list">
          {timelineItems.map((item) =>
            item.kind === "now" ? (
              <LifeStreamNowMarker
                key="now-marker"
                label={item.label}
                gapPx={item.gapPx}
              />
            ) : (
              <CardErrorBoundary key={item.id} cardId={item.id}>
                <LifeMessageRow
                  cardId={item.id}
                  index={item.cardIndex}
                  gapPx={item.gapPx}
                  onCancel={cancel}
                  onRetry={retry}
                  onClarify={clarify}
                />
              </CardErrorBoundary>
            ),
          )}
        </section>
      ) : (
        <div className="life-stream-empty">
          No entries yet. Start logging your day!
        </div>
      )}
    </div>
  );
}
