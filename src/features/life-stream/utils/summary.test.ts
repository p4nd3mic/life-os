import { describe, expect, it } from "vitest";
import { buildCollapsedSummary } from "./summary";

describe("buildCollapsedSummary", () => {
  it("builds section + bullet summaries", () => {
    const markdown = `✅ Why Ep 5 feels like "the real show starts here"
1) It's the first time the past stops being "vibes" and becomes plot
2) Vicious arrives as an actual gravitational force
3) The syndicate + Spike's history becomes the spine
4) The church sequence is basically a mission statement

🤔 "It should've been Episode 1" — the tradeoff
- Gains: main-plot momentum, clear antagonist, serialized feel
- Loses: bait-and-switch + tonal reset
`;

    const summary = buildCollapsedSummary(markdown);
    expect(summary).toMatch(/### Why Ep 5 feels/);
    expect(summary).toMatch(/### 1\) It's the first time/);
    expect(summary).toMatch(/### 2\) Vicious arrives/);
    expect(summary).toMatch(/### 3\) The syndicate \+ Spike's history becomes the spine/);
    expect(summary).not.toMatch(/tradeoff/);
  });

  it("treats top-level numbered lines as headers consistently", () => {
    const markdown = `1) First section
supporting line
2) Second section
another detail
3) Third section`;

    const summary = buildCollapsedSummary(markdown);
    expect(summary).toContain("### 1) First section");
    expect(summary).toContain("### 2) Second section");
    expect(summary).toContain("### 3) Third section");
    expect(summary).not.toContain("- 2) Second section");
  });

  it("keeps nested numbered lists as list items, not headers", () => {
    const markdown = `### Parent section
  1) nested one
  2) nested two`;

    const summary = buildCollapsedSummary(markdown);
    expect(summary).toContain("### Parent section");
    expect(summary).toContain("- nested one");
    expect(summary).toContain("- nested two");
  });

  it("handles emoji-prefixed numbered headers", () => {
    const markdown = `🟥 1) First pass
🟥 2) Second pass
🟥 3) Third pass`;

    const summary = buildCollapsedSummary(markdown);
    expect(summary).toContain("### 1) First pass");
    expect(summary).toContain("### 2) Second pass");
    expect(summary).toContain("### 3) Third pass");
  });

  it("treats top-level bullet-prefixed numbered lines as headers", () => {
    const markdown = `- 🟥 1) First pass
• 🟥 2) Second pass
• 🟥 3) Third pass`;

    const summary = buildCollapsedSummary(markdown);
    expect(summary).toContain("### 1) First pass");
    expect(summary).toContain("### 2) Second pass");
    expect(summary).toContain("### 3) Third pass");
  });

  it("treats bold-wrapped numbered lines as headers", () => {
    const markdown = `**🟥 1) First pass** 🔥
text
**🟥 2) Second pass** 🗡️
text
**🟥 3) Third pass** ✅`;

    const summary = buildCollapsedSummary(markdown);
    expect(summary).toContain("### 1) First pass");
    expect(summary).toContain("### 2) Second pass");
    expect(summary).toContain("### 3) Third pass");
  });
});
