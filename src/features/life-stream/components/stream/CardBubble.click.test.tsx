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
  layoutMode: overrides.layoutMode,
  causal: overrides.causal,
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
        onRestructure={() => {}}
      />,
    );

    const article = container.querySelector("article");
    expect(article).toBeTruthy();
    fireEvent.click(article!);

    expect(screen.getByText("Expanded content")).toBeTruthy();
  });

  it("expands transcript when clicking the cause node in graph mode", () => {
    const card = makeCard({
      layoutMode: "cause_effect",
      summary: undefined,
      causal: {
        leftNodes: [
          {
            id: "left-1",
            text: "Cause text",
            title: "Cause title",
            bullets: ["Cause bullet"],
            details: "Cause details",
            role: "cause",
          },
        ],
        rightNodes: [
          {
            id: "right-1",
            text: "Effect text",
            title: "Effect title",
            bullets: ["Effect bullet"],
            details: "Effect details",
            role: "effect",
          },
        ],
        links: [{ fromId: "left-1", toId: "right-1" }],
        layout: { visibleRightCount: 3, topLinkLimit: 3, expanded: false },
      },
    });

    const { container } = render(
      <CardBubble
        card={card}
        onCancel={() => {}}
        onRetry={() => {}}
        onClarify={() => {}}
        onRestructure={() => {}}
      />,
    );

    const graphSection = container.querySelector(".life-graph-card");
    expect(graphSection).toBeTruthy();

    const leftNode = container.querySelector(
      ".life-causal-card__lane--left .life-causal-card__node",
    );
    expect(leftNode).toBeTruthy();
    fireEvent.click(leftNode!);
    const graphTranscript = container.querySelector(".life-graph-card__transcript");
    expect(graphTranscript).toBeTruthy();
    expect(graphTranscript?.textContent?.includes("Original Input")).toBe(true);
    expect(graphTranscript?.textContent?.includes("Codex Response")).toBe(true);
    expect(graphTranscript?.textContent?.includes("Expanded content")).toBe(true);
  });

  it("hides right-side inline image prompts by default in graph mode", () => {
    const card = makeCard({
      layoutMode: "cause_effect",
      summary: undefined,
      causal: {
        leftNodes: [
          {
            id: "left-1",
            text: "Cause text",
            title: "Cause title",
            role: "cause",
            image: { status: "ready", url: "https://example.com/cause.jpg" },
          },
        ],
        rightNodes: [
          {
            id: "right-1",
            text: "Reason text",
            title: "Reason title",
            role: "response",
          },
        ],
        links: [{ fromId: "left-1", toId: "right-1" }],
      },
    });

    const { container } = render(
      <CardBubble
        card={card}
        onCancel={() => {}}
        onRetry={() => {}}
        onClarify={() => {}}
        onRestructure={() => {}}
      />,
    );

    expect(container.textContent?.includes("No image yet")).toBe(false);
    expect(container.textContent?.includes("Set image")).toBe(false);
  });

  // Selection-safe behavior is enforced in the component, but not unit-tested
  // due to jsdom limitations around Selection APIs.
});
