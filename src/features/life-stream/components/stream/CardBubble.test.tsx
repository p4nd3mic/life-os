// @vitest-environment jsdom
import { render, screen } from "@testing-library/react";
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
  image: overrides.image,
  originalInput: overrides.originalInput,
});

describe("CardBubble", () => {
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
      />,
    );

    expect(screen.queryByText("Response")).toBeNull();
    expect(screen.getByText(/Header/i)).toBeTruthy();
    expect(screen.getByText(/Point one/i)).toBeTruthy();
  });
});
