import { describe, expect, it } from "vitest";
import type { StreamCard } from "../types";
import { resolveCardTitleWithIcon } from "./cardTitle";

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
  summary: overrides.summary,
  image: overrides.image,
});

describe("resolveCardTitleWithIcon", () => {
  it("prefixes the title with emoji once", () => {
    const card = makeCard({
      emoji: "🎬",
      title: "Favorite Cowboy Bebop episode 5",
    });

    expect(resolveCardTitleWithIcon(card)).toBe(
      "🎬 Favorite Cowboy Bebop episode 5",
    );
  });

  it("does not double-prefix when emoji already present", () => {
    const card = makeCard({
      emoji: "🎬",
      title: "🎬 Favorite Cowboy Bebop episode 5",
    });

    expect(resolveCardTitleWithIcon(card)).toBe(
      "🎬 Favorite Cowboy Bebop episode 5",
    );
  });

  it("uses domain fallback when emoji is missing", () => {
    const card = makeCard({
      emoji: "",
      domain: "delivery",
      title: "Delivery: Panera $14 / 3.2 mi",
    });

    expect(resolveCardTitleWithIcon(card)).toBe(
      "🚗 Delivery: Panera $14 / 3.2 mi",
    );
  });

  it("uses card type label when title is missing", () => {
    const card = makeCard({
      emoji: "",
      domain: "nutrition",
      title: "Response",
      cardType: "meal",
      originalInput: "ate eggs and toast",
    });

    expect(resolveCardTitleWithIcon(card)).toBe("🍽️ ate eggs and toast");
  });

  it("rebuilds truncated titles from input", () => {
    const card = makeCard({
      emoji: "🎬",
      title: "So, I think episode 5 of Cowboy Bebop is my",
      originalInput:
        "So, I think episode 5 of Cowboy Bebop is my favorite. It's the first main story episode.",
    });

    expect(resolveCardTitleWithIcon(card)).toBe(
      "🎬 episode 5 of Cowboy Bebop is my favorite",
    );
  });
});
