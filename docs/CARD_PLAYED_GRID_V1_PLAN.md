# Card Played Grid — V1

**Status:** Implemented (2026-02)
**Replaces:** Fan layout mode in vertical right lane
**Keeps:** Timeline mode + improved statement card stage styling
**Superseded by:** `docs/CARD_PLAYED_GRID_V2_IMAGE_LEAD_PLAN.md` (summary-first lead + top art slot)

---

## Goal

Replace the curved fan layout with a flat, table-style card board that feels like cards already played:

- 3 columns desktop, 2 columns tablet, 1 column phone
- no card image/icon block
- each card is self-contained with title + summary + numbered bullets
- no separate detail panel needed
- retain rank-based interaction, context menu, keyboard shortcuts, and selection state

---

## Visual grammar

- **Card shape:** premium dark card, rounded corners, warm noise texture, subtle depth
- **Color rhythm:** rank-order cycle `Gold → Green → Blue` repeating by displayed order
- **Hierarchy:**
  1. Title (2 lines)
  2. Summary (2-3 lines, italic)
  3. Numbered bullets (semantic payload)
- **Selection:** subtle lift/glow; no modal/detail panel

---

## Interaction model

- Click/tap: select card (toggle behavior)
- Right-click / long-press: open node context actions
- Keyboard:
  - `1..9` selects by rank position
  - Arrow keys navigate board cards in grid-style movement
  - `Esc` clears selection

---

## Files

### Added
- `src/features/life-stream/components/stream/CardBoardLayout.tsx`
- `src/features/life-stream/components/stream/CardBoardLayout.css`
- `src/features/life-stream/components/stream/BoardCard.tsx`
- `src/features/life-stream/components/stream/BoardCard.css`

### Updated
- `src/features/life-stream/components/stream/CauseEffectCard.tsx`
- `src/features/life-stream/components/stream/CauseEffectCard.css`
- `src/features/life-stream/components/stream/CauseEffectCard.orientation.test.tsx`
- `src/features/life-stream/components/stream/CardBubble.test.tsx`

### Removed (superseded)
- `src/features/life-stream/components/stream/CardFanLayout.tsx`
- `src/features/life-stream/components/stream/CardFanLayout.css`
- `src/features/life-stream/components/stream/FanCard.tsx`
- `src/features/life-stream/components/stream/FanCard.css`

---

## Verification checklist

- [x] Toggle switches between timeline and played-card board
- [x] Board renders wrapped rows (3/2/1 breakpoints)
- [x] No card image/icon area in board cards
- [x] Cards show title + summary + numbered bullets directly
- [x] No fan detail panel path in board mode
- [x] Context menu + keyboard actions still work
- [x] Statement stage styling preserved and integrated
- [x] Typecheck + orientation tests + build pass

---

## Screenshot location

`docs/screenshots/card-played-grid-v1/`

Includes desktop, iPad, and iPhone captures on **Mon, Feb 2** (Cowboy Bebop-heavy card set).
