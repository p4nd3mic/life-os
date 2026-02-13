# Card Played Grid — V2 (Image + Lead Merge)

**Status:** Implemented (2026-02)  
**Builds on:** `docs/CARD_PLAYED_GRID_V1_PLAN.md`  
**Focus:** Right-lane board cards now use a summary-first lead line plus top art slot.

---

## Goal

Upgrade played cards into stronger TCG-like visual cards while preserving readability:

- Merge title + summary into one **lead line** (summary-first).
- Add a top **4:3 art window** on every right-lane card.
- Keep numbered bullets as the core semantic payload.
- Add missing-art placeholder with **Fetch image** CTA.
- Maintain rank selection, context menu, and keyboard controls.

---

## Anatomy (top → bottom)

1. **Art slot (4:3)**  
   - `image.status === ready` + `image.url` → show artwork.
   - otherwise show premium placeholder + fetch CTA.
2. **Lead line**  
   - summary-first merged line, enriched with title specificity only when needed.
3. **Divider**
4. **Numbered bullets**

---

## Lead-line merge strategy

- Base: `summaryLine` if present and non-generic.
- Anchors extracted from title/headline:
  - quoted terms
  - episode tokens (`Episode 5`, `Ep 5`)
  - proper multi-word names
- If base lacks an anchor, append compact suffix:
  - `… — anchored to “{anchor}.”`
- Clamp to ~180 chars at word boundaries.
- Normalize number spacing (`Episode5` → `Episode 5`, `Episode1,2` → `Episode 1, 2`).
- Dedupe against first bullet if near-identical.

---

## Files touched

- `src/features/life-stream/components/stream/CauseEffectCard.tsx`
- `src/features/life-stream/components/stream/CauseEffectCard.css`
- `src/features/life-stream/components/stream/CardBoardLayout.tsx`
- `src/features/life-stream/components/stream/CardBoardLayout.css`
- `src/features/life-stream/components/stream/BoardCard.tsx`
- `src/features/life-stream/components/stream/BoardCard.css`
- `src/features/life-stream/components/stream/CauseEffectCard.orientation.test.tsx`
- `src/features/life-stream/components/stream/BoardCard.test.tsx`

---

## Verification targets

- Desktop: 3 columns, readable lead + bullets, art window visible.
- iPad mini class: 2 columns, no clipping.
- iPhone class: 1 column, tap targets intact.
- Mon, Feb 2 Cowboy Bebop fixture:
  - lead merge quality
  - number spacing fix retained
  - fetch CTA appears on missing art.

