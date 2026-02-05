// @vitest-environment jsdom
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { StreamCard } from "../../types";
import { CardBubble } from "./CardBubble";

const makeCard = (overrides: Partial<StreamCard> = {}): StreamCard => ({
  id: overrides.id ?? "card-1",
  occurredAt: overrides.occurredAt ?? "2026-02-01T10:00:00",
  createdAt: overrides.createdAt ?? "2026-02-01T10:00:00",
  updatedAt: overrides.updatedAt ?? "2026-02-01T10:00:02",
  version: overrides.version ?? 1,
  cardType: overrides.cardType ?? "generic",
  domain: overrides.domain ?? "general",
  emoji: overrides.emoji ?? "📝",
  state: overrides.state ?? "complete",
  title: overrides.title ?? "Response",
  summary: overrides.summary ?? "Summary",
  expanded: overrides.expanded ?? {
    originalInput: "test",
    sections: [{ title: "Codex Response", body: "Expanded content" }],
    actions: [],
  },
});

describe("CardBubble expand click behavior", () => {
  it("expands when clicking the bubble", () => {
    const card = makeCard();
    const { container } = render(
      <CardBubble
        card={card}
        onCancel={() => {}}
        onRetry={() => {}}
        onClarify={() => {}}
      />,
    );

    const article = container.querySelector("article");
    expect(article).toBeTruthy();
    fireEvent.click(article!);

    expect(screen.getByText("Expanded content")).toBeTruthy();
  });

  // Selection-safe behavior is enforced in the component, but not unit-tested
  // due to jsdom limitations around Selection APIs.
});
