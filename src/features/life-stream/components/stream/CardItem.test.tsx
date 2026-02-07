// @vitest-environment jsdom
import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import type { StreamCard } from "../../types";
import { streamStore } from "../../state/streamStore";
import { CardItem } from "./CardItem";

const makeCard = (overrides: Partial<StreamCard> = {}): StreamCard => ({
  id: overrides.id ?? "card-1",
  occurredAt: overrides.occurredAt ?? "2026-02-01T10:00:00",
  createdAt: overrides.createdAt ?? "2026-02-01T10:00:00",
  updatedAt: overrides.updatedAt ?? "2026-02-01T10:00:02",
  version: overrides.version ?? 2,
  cardType: overrides.cardType ?? "generic",
  domain: overrides.domain ?? "general",
  emoji: overrides.emoji ?? "📝",
  state: overrides.state ?? "complete",
  title: overrides.title ?? "Test Card",
  summary: overrides.summary ?? "Summary",
  durationMs: overrides.durationMs ?? 2000,
  originalInput: overrides.originalInput,
});

describe("CardItem", () => {
  beforeEach(() => {
    streamStore.loadCards([]);
    streamStore.setDate("2026-02-01");
  });

  it("renders the persisted duration for completed cards", () => {
    const card = makeCard();
    streamStore.addCard(card);

    render(
      <CardItem
        cardId={card.id}
        onCancel={() => {}}
        onRetry={() => {}}
        onClarify={() => {}}
      />,
    );

    expect(screen.getByText("Done in 0:02")).toBeTruthy();
  });

  it("uses icon-prefixed title fallback when generic", () => {
    const card = makeCard({
      title: "Response",
      originalInput: "favorite cowboy bebop episode 5",
      emoji: "🎬",
    });
    streamStore.addCard(card);

    render(
      <CardItem
        cardId={card.id}
        onCancel={() => {}}
        onRetry={() => {}}
        onClarify={() => {}}
      />,
    );

    expect(
      screen.getByText(/🎬 Favorite Cowboy Bebop Episode 5/i),
    ).toBeTruthy();
  });
});
