// @vitest-environment jsdom
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { StreamCard } from "../../types";
import { ExpandedCard } from "./ExpandedCard";

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
  title: overrides.title ?? "Test",
  summary: overrides.summary ?? "Summary",
  durationMs: overrides.durationMs ?? 2000,
  expanded: overrides.expanded ?? {
    originalInput: "testing 123",
    sections: [
      { title: "Codex Decision JSON", body: "{\"intent\":\"log_meal\"}" },
    ],
    actions: [],
  },
});

describe("ExpandedCard", () => {
  it("renders Codex Decision JSON as a collapsible section", () => {
    const card = makeCard();
    render(<ExpandedCard card={card} onCollapse={() => {}} />);

    expect(screen.getByText("Codex Decision JSON")).toBeTruthy();
    expect(screen.getByText("{\"intent\":\"log_meal\"}")).toBeTruthy();
  });
});
