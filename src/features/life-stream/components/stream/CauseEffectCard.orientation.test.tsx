// @vitest-environment jsdom
import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";
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
      { id: "right-1", text: "Why 1", role: "response", headline: "Faster lock-in" },
      { id: "right-2", text: "Why 2", role: "response", headline: "Mood boost" },
      { id: "right-3", text: "Why 3", role: "response", headline: "Story focus" },
      { id: "right-4", text: "Why 4", role: "response", headline: "Long-tail motivation" },
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
  it("renders stacked lanes and keeps dense fanout compact in a single overflow card", () => {
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

    const lanes = container.querySelector(".life-causal-card__lanes--vertical");
    expect(lanes).toBeTruthy();

    const overflow = container.querySelector(
      ".life-causal-card__lane--right .life-causal-card__node.is-overflow-summary",
    );
    expect(overflow).toBeTruthy();
    expect(overflow?.textContent?.includes("More reasons")).toBe(true);
  });

  it("adds stagger offsets to right-side nodes in vertical mode", () => {
    const { container } = render(
      <CauseEffectCard
        cardId="card-stagger"
        cardType="thought"
        cardTitle="Stagger"
        causal={makeCausalPayload()}
        layoutOrientation="vertical"
        onRestructure={() => {}}
      />,
    );

    const rightNodes = container.querySelectorAll(
      ".life-causal-card__lane--right .life-causal-card__node",
    );
    expect(rightNodes.length).toBeGreaterThanOrEqual(3);

    const firstStyle = rightNodes[0]?.getAttribute("style") ?? "";
    const secondStyle = rightNodes[1]?.getAttribute("style") ?? "";
    expect(firstStyle).toContain("--life-node-stagger-y");
    expect(secondStyle).toContain("--life-node-stagger-y");
    expect(firstStyle).not.toEqual(secondStyle);
  });
});
