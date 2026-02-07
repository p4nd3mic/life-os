// @vitest-environment jsdom
import { cleanup, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
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
  image: overrides.image,
  originalInput: overrides.originalInput,
});

describe("CardBubble", () => {
  afterEach(() => {
    cleanup();
  });

  it("does not render placeholder when image is missing", () => {
    const card = makeCard({
      image: { status: "missing" },
    });

    render(
      <CardBubble
        card={card}
        onCancel={() => {}}
        onRetry={() => {}}
        onClarify={() => {}}
        onRestructure={() => {}}
      />,
    );

    expect(screen.queryByText(/no image/i)).toBeNull();
  });

  it("renders bullet summary without response label", () => {
    const card = makeCard({
      summary: "## Header\n- Point one\n- Point two",
      originalInput: "Some input",
    });

    render(
      <CardBubble
        card={card}
        onCancel={() => {}}
        onRetry={() => {}}
        onClarify={() => {}}
        onRestructure={() => {}}
      />,
    );

    expect(screen.queryByText("Response")).toBeNull();
    expect(screen.getByText(/Header/i)).toBeTruthy();
    expect(screen.getByText(/Point one/i)).toBeTruthy();
  });

  it("renders cause/effect layout controls for dense right nodes", () => {
    const card = makeCard({
      layoutMode: "cause_effect",
      causal: {
        leftNodes: [
          {
            id: "left-1",
            text: "Evening walk",
            role: "cause",
          },
        ],
        rightNodes: [
          { id: "right-1", text: "Episode 1", role: "effect" },
          { id: "right-2", text: "Episode 2", role: "effect" },
          { id: "right-3", text: "Episode 3", role: "effect" },
          { id: "right-4", text: "Episode 4", role: "effect" },
        ],
        links: [
          { fromId: "left-1", toId: "right-1", strength: 1 },
          { fromId: "left-1", toId: "right-2", strength: 0.95 },
          { fromId: "left-1", toId: "right-3", strength: 0.9 },
          { fromId: "left-1", toId: "right-4", strength: 0.7 },
        ],
        layout: {
          visibleRightCount: 3,
          topLinkLimit: 2,
          expanded: false,
        },
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

    const graph = container.querySelector(".life-graph-card");
    expect(graph).toBeTruthy();
    const scoped = within(graph as HTMLElement);

    expect(scoped.getByText(/Evening walk/i)).toBeTruthy();
    expect(scoped.getAllByText(/^Episode 1$/i).length).toBeGreaterThanOrEqual(1);
    expect(scoped.getAllByText(/^Episode 2$/i).length).toBeGreaterThanOrEqual(1);
    expect(scoped.getAllByText(/^Episode 3$/i).length).toBeGreaterThanOrEqual(1);
    expect(scoped.getByText(/More effects/i)).toBeTruthy();
    expect(scoped.getByText(/Episode 4/i)).toBeTruthy();
    expect(scoped.queryByRole("button", { name: /show all outcomes/i })).toBeNull();
  });

  it("delivery sessions show latest effects in collapsed mode", () => {
    const card = makeCard({
      cardType: "delivery_session",
      layoutMode: "cause_effect",
      causal: {
        leftNodes: [
          {
            id: "left-1",
            text: "Dinner session",
            role: "action",
          },
        ],
        rightNodes: [
          { id: "right-1", text: "Order #1", role: "reward" },
          { id: "right-2", text: "Order #2", role: "reward" },
          { id: "right-3", text: "Order #3", role: "reward" },
          { id: "right-4", text: "Order #4", role: "reward" },
        ],
        links: [
          { fromId: "left-1", toId: "right-1", strength: 0.8 },
          { fromId: "left-1", toId: "right-2", strength: 0.85 },
          { fromId: "left-1", toId: "right-3", strength: 0.9 },
          { fromId: "left-1", toId: "right-4", strength: 1 },
        ],
        layout: {
          visibleRightCount: 3,
          topLinkLimit: 2,
          expanded: false,
        },
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

    const graph = container.querySelector(".life-graph-card");
    expect(graph).toBeTruthy();
    const scoped = within(graph as HTMLElement);

    expect(scoped.getAllByText(/Order #2/i).length).toBeGreaterThanOrEqual(1);
    expect(scoped.getAllByText(/Order #3/i).length).toBeGreaterThanOrEqual(1);
    expect(scoped.getAllByText(/Order #4/i).length).toBeGreaterThanOrEqual(1);
    expect(scoped.getByText(/More outcomes/i)).toBeTruthy();
    expect(scoped.getByText(/Order #1/i)).toBeTruthy();
  });
});
