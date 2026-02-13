// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { CausalNode } from "../../types";
import { BoardCard, type BoardCardResolvedNode } from "./BoardCard";

function makeNode(overrides: Partial<CausalNode> = {}): CausalNode {
  return {
    id: overrides.id ?? "node-1",
    text: overrides.text ?? "Node text",
    role: overrides.role ?? "response",
    ...overrides,
  };
}

function renderBoardCard(
  resolved: BoardCardResolvedNode,
  overrides: { node?: CausalNode; onSelect?: ReturnType<typeof vi.fn>; onRequestImage?: ReturnType<typeof vi.fn> } = {},
) {
  const onSelect = overrides.onSelect ?? vi.fn();
  const onOpenContextMenu = vi.fn();
  const onRequestImage = overrides.onRequestImage;
  const node = overrides.node ?? makeNode();

  const result = render(
    <BoardCard
      node={node}
      rank={1}
      colorVariant="gold"
      resolved={resolved}
      isSelected={false}
      setRef={() => {}}
      onSelect={onSelect}
      onOpenContextMenu={onOpenContextMenu}
      onRequestImage={onRequestImage}
    />,
  );

  return { ...result, onSelect, onOpenContextMenu, onRequestImage, node };
}

describe("BoardCard", () => {
  afterEach(() => {
    cleanup();
  });

  it("renders lead text and bullets as the primary payload", () => {
    renderBoardCard({
      leadLine: "Name accuracy matters because this episode is a major arc landmark.",
      bullets: ["Point one", "Point two"],
      imageStatus: "missing",
      ariaTitle: "Ballad of Fallen Angels",
    });

    expect(screen.getByRole("button", { name: /card 1: ballad of fallen angels/i })).toBeTruthy();
    expect(screen.getByText(/Name accuracy matters/i)).toBeTruthy();
    expect(screen.getByText("Point one")).toBeTruthy();
    expect(screen.getByText("Point two")).toBeTruthy();
  });

  it("shows fetch CTA for missing art and calls image request without toggling selection", () => {
    const onSelect = vi.fn();
    const onRequestImage = vi.fn();
    const node = makeNode({ id: "node-fetch" });
    renderBoardCard(
      {
        leadLine: "Lead line",
        bullets: ["Point one"],
        imageStatus: "missing",
        ariaTitle: "Need image",
      },
      { onSelect, onRequestImage, node },
    );

    const fetchButton = screen.getByRole("button", { name: /fetch image/i });
    fireEvent.click(fetchButton);

    expect(onRequestImage).toHaveBeenCalledWith("node-fetch");
    expect(onSelect).not.toHaveBeenCalled();
  });

  it("renders ready art image when present", () => {
    renderBoardCard({
      leadLine: "Lead line",
      bullets: ["Point one"],
      imageStatus: "ready",
      imageSrc: "https://example.com/art.jpg",
      ariaTitle: "Ready image card",
    });

    const image = screen.getByRole("img", { name: /ready image card/i });
    expect(image).toBeTruthy();
    expect(screen.queryByRole("button", { name: /fetch image/i })).toBeNull();
  });
});

