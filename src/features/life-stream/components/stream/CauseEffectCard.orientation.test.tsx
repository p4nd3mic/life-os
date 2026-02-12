// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import type { CausalCardContent } from "../../types";
import { CauseEffectCard } from "./CauseEffectCard";

function makeCausalPayload(): CausalCardContent {
  return {
    leftNodes: [
      {
        id: "left-1",
        text: "Evening walk with P90X warmup",
        role: "cause",
        headline: "Walk + P90X set the tone",
      },
    ],
    rightNodes: [
      {
        id: "right-1",
        text: "Why 1",
        role: "response",
        headline: "Faster lock-in",
        summaryLine: "The structure helps the viewer lock into stakes quickly.",
        bullets: ["Opens character history early", "Frames immediate conflict"],
        rank: 2,
      },
      {
        id: "right-2",
        text: "Why 2",
        role: "response",
        headline: "Mood boost",
        summaryLine: "Tone contrast deepens the emotional impact.",
        bullets: ["Style shifts from cool to personal", "Makes the turn memorable"],
        rank: 1,
      },
      {
        id: "right-3",
        text: "Why 3",
        role: "response",
        headline: "Story focus",
        summaryLine: "Narrative signal becomes clearer after setup.",
        bullets: ["Improves arc readability", "Raises anticipation"],
        rank: 3,
      },
      {
        id: "right-4",
        text: "Why 4",
        role: "response",
        headline: "Long-tail motivation",
        summaryLine: "Keeps momentum through later episodes.",
        bullets: ["Plants future payoffs", "Strengthens continuity"],
        rank: 4,
      },
    ],
    links: [
      { fromId: "left-1", toId: "right-1", strength: 1 },
      { fromId: "left-1", toId: "right-2", strength: 0.9 },
      { fromId: "left-1", toId: "right-3", strength: 0.85 },
      { fromId: "left-1", toId: "right-4", strength: 0.8 },
    ],
    semanticMode: "statement_why",
  };
}

describe("CauseEffectCard vertical orientation", () => {
  afterEach(() => {
    cleanup();
  });

  it("renders played cards as the default right-lane layout", () => {
    const { container } = render(
      <CauseEffectCard
        cardId="card-vertical"
        cardType="thought"
        cardTitle="Cowboy Bebop thoughts"
        causal={makeCausalPayload()}
        layoutOrientation="vertical"
        onRestructure={() => {}}
      />,
    );

    const root = container.querySelector(".life-causal-card--vertical");
    expect(root).toBeTruthy();

    expect(container.querySelectorAll(".board-card").length).toBe(4);
    expect(container.querySelector(".life-timeline-section")).toBeNull();
    expect(container.querySelector(".life-causal-card__layout-toggle")).toBeNull();
    expect(screen.queryByText("💡 WHY")).toBeNull();
  });

  it("orders played cards by rank", () => {
    const { container } = render(
      <CauseEffectCard
        cardId="card-rank"
        cardType="thought"
        cardTitle="Rank order"
        causal={makeCausalPayload()}
        layoutOrientation="vertical"
        onRestructure={() => {}}
      />,
    );

    const cards = Array.from(
      container.querySelectorAll(".board-card"),
    );
    const ids = cards.map((card) => card.getAttribute("data-node-id"));

    expect(ids).toEqual(["right-2", "right-1", "right-3", "right-4"]);
  });

  it("keeps all core content on cards without detail panel", () => {
    const { container } = render(
      <CauseEffectCard
        cardId="card-content"
        cardType="thought"
        cardTitle="Card content"
        causal={makeCausalPayload()}
        layoutOrientation="vertical"
        onRestructure={() => {}}
      />,
    );

    expect(container.querySelector(".fan-detail-panel")).toBeNull();
    expect(container.querySelector(".board-card__title")?.textContent).toContain("Mood boost");
    expect(container.querySelector(".board-card__summary")?.textContent).toContain("Tone contrast");
    expect(container.querySelectorAll(".board-card__bullets li").length).toBeGreaterThan(0);
  });

  it("selects a board card without opening extra detail UI", () => {
    const { container } = render(
      <CauseEffectCard
        cardId="card-selection"
        cardType="thought"
        cardTitle="Selection"
        causal={makeCausalPayload()}
        layoutOrientation="vertical"
        onRestructure={() => {}}
      />,
    );

    const rankOneCard = container.querySelector(
      '.board-card[data-node-id="right-2"]',
    ) as HTMLElement | null;
    expect(rankOneCard).toBeTruthy();
    if (!rankOneCard) {
      throw new Error("expected rank 1 board card");
    }

    fireEvent.click(rankOneCard);

    expect(
      container.querySelector('.board-card.is-selected[data-node-id="right-2"]'),
    ).toBeTruthy();
    expect(container.querySelector(".fan-detail-panel")).toBeNull();
  });

  it("supports arrow key navigation in board mode", () => {
    const { container } = render(
      <CauseEffectCard
        cardId="card-arrows"
        cardType="thought"
        cardTitle="Arrow nav"
        causal={makeCausalPayload()}
        layoutOrientation="vertical"
        onRestructure={() => {}}
      />,
    );

    fireEvent.keyDown(window, { key: "ArrowRight" });
    expect(
      container.querySelector('.board-card.is-selected[data-node-id="right-2"]'),
    ).toBeTruthy();

    fireEvent.keyDown(window, { key: "ArrowRight" });
    expect(
      container.querySelector('.board-card.is-selected[data-node-id="right-1"]'),
    ).toBeTruthy();

    fireEvent.keyDown(window, { key: "ArrowLeft" });
    expect(
      container.querySelector('.board-card.is-selected[data-node-id="right-2"]'),
    ).toBeTruthy();
  });

  it("keeps rank keyboard shortcuts and context menu working in board mode", () => {
    const { container } = render(
      <CauseEffectCard
        cardId="card-context-board"
        cardType="thought"
        cardTitle="Context actions"
        causal={makeCausalPayload()}
        layoutOrientation="vertical"
        onRestructure={() => {}}
      />,
    );

    fireEvent.keyDown(window, { key: "3" });
    expect(
      container.querySelector('.board-card.is-selected[data-node-id="right-3"]'),
    ).toBeTruthy();

    const selectedCard = container.querySelector(
      '.board-card[data-node-id="right-3"]',
    ) as HTMLElement | null;
    expect(selectedCard).toBeTruthy();
    if (!selectedCard) {
      throw new Error("expected selected board card");
    }

    fireEvent.contextMenu(selectedCard);
    expect(screen.getByRole("menu", { name: "Node actions" })).toBeTruthy();
    expect(screen.getByRole("menuitem", { name: "✂️ Split cause" })).toBeTruthy();
    expect(screen.getByRole("menuitem", { name: "🧩 Merge effects" })).toBeTruthy();
  });

  it("uses CSS spine instead of SVG connectors in vertical mode", () => {
    const { container } = render(
      <CauseEffectCard
        cardId="card-geometry"
        cardType="thought"
        cardTitle="Connector geometry"
        causal={makeCausalPayload()}
        layoutOrientation="vertical"
        onRestructure={() => {}}
      />,
    );

    const allPaths = container.querySelectorAll(".life-causal-card__path");
    expect(allPaths.length).toBe(0);

    const rightLane = container.querySelector(".life-causal-card__lane--right");
    expect(rightLane).toBeTruthy();
  });
});
