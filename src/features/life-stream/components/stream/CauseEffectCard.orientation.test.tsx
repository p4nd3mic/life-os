// @vitest-environment jsdom
import { fireEvent, render } from "@testing-library/react";
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

  it("auto-expands the top-ranked right node and toggles expansion on click", () => {
    const { container } = render(
      <CauseEffectCard
        cardId="card-expand"
        cardType="thought"
        cardTitle="Ranked expansion"
        causal={makeCausalPayload()}
        layoutOrientation="vertical"
        onRestructure={() => {}}
      />,
    );

    const selectedNode = container.querySelector(
      ".life-causal-card__lane--right .life-causal-card__node.is-selected",
    );
    expect(selectedNode).toBeTruthy();
    expect(selectedNode?.querySelector(".life-causal-card__node-rank")?.textContent).toBe("1");

    const detailTier = container.querySelector(".life-causal-card__detail-tier");
    expect(detailTier).toBeTruthy();
    expect(detailTier?.textContent?.includes("Mood boost")).toBe(true);

    if (!selectedNode) {
      throw new Error("expected selected node");
    }
    fireEvent.click(selectedNode);
    const detailTierAfterCollapse = container.querySelector(".life-causal-card__detail-tier");
    expect(detailTierAfterCollapse).toBeNull();
  });

  it("keeps right-side cards concise and moves bullets into the third layer", () => {
    const { container } = render(
      <CauseEffectCard
        cardId="card-concise"
        cardType="thought"
        cardTitle="Concise layer"
        causal={makeCausalPayload()}
        layoutOrientation="vertical"
        onRestructure={() => {}}
      />,
    );

    const rightLaneBullets = container.querySelector(
      ".life-causal-card__lane--right .life-causal-card__node-bullets",
    );
    expect(rightLaneBullets).toBeNull();

    const detailNodes = container.querySelectorAll(".life-causal-card__detail-node");
    expect(detailNodes.length).toBeGreaterThan(0);
  });

  it("renders trunk connector paths for ranked fanout links", () => {
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
    expect(allPaths.length).toBeGreaterThan(0);

    const trunkPaths = container.querySelectorAll(".life-causal-card__path.is-trunk");
    expect(trunkPaths.length).toBeGreaterThan(0);
  });
});
