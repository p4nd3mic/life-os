# Card Fan Layout — V2 Implementation Plan

**Status:** Superseded by Played Grid V1
> Superseded note: Right-lane fan mode has been replaced by flat board mode. See `docs/CARD_PLAYED_GRID_V1_PLAN.md` for the current spec.
**Inspiration:** Slay the Spire / Monster Train / Hearthstone / Magic: The Gathering
**Toggle:** Coexists with Spine Flow (timeline) — user can switch between modes

---

## Table of Contents

1. [Visual Design Spec](#1-visual-design-spec)
2. [Card Anatomy](#2-card-anatomy)
3. [Fan Layout Math](#3-fan-layout-math)
4. [Interaction Model](#4-interaction-model)
5. [Architecture & Files](#5-architecture--files)
6. [Step-by-Step Implementation](#6-step-by-step-implementation)
7. [CSS Reference](#7-css-reference)
8. [AI Implementor Notes](#8-ai-implementor-notes)
9. [Verification Checklist](#9-verification-checklist)

---

## 1. Visual Design Spec

### Overall Composition

```
┌──────────────────────────────────────────────────────────┐
│                                                          │
│           ┌──────────────────────┐                       │
│           │  🧩 STATEMENT CARD   │  ← Parent card        │
│           │  (glassy, centered)  │     (existing style)  │
│           └──────────────────────┘                       │
│                      │                                   │
│                    💡 WHY                                 │
│                      │                                   │
│     ╭─────╮  ╭─────╮  ╭─────╮  ╭─────╮  ╭─────╮       │
│    │  5  │ │  3  │ │  1  │ │  2  │ │  4  │       │
│    │     │ │     │ │     │ │     │ │     │       │
│    │ ╲   │ │╲    │ │     │ │   ╱ │ │  ╱  │       │
│     ╰──╲──╯  ╰─╲──╯  ╰─────╯  ╰──╱─╯  ╰─╱───╯       │
│         ╲      ╲                ╱      ╱                │
│          ╰──────╰──────────────╯──────╯                  │
│                  (fan arc)                               │
│                                                          │
└──────────────────────────────────────────────────────────┘
```

**Key principles:**
- Cards are arranged in a **semicircular arc** below the parent
- Rank 1 (most important) is **center and highest** in the arc
- Cards fan outward with rotation, lower ranks toward edges
- Overlapping slightly like a hand of playing cards
- On hover: card **lifts**, **un-rotates**, **scales up 1.3x**, and shows full content
- On click/tap: card **stays expanded** in a detail zone above the fan

### Color Palette (Inherited from Spine Flow + Card Game Inspiration)

| Element | Color | Reference |
|---------|-------|-----------|
| Card frame (default) | `rgba(28, 26, 22, 0.85)` | Dark warm, semi-transparent |
| Card frame border | `rgba(110, 231, 183, 0.25)` | Emerald glow |
| Card frame border (rank 1) | `rgba(251, 191, 36, 0.5)` | Gold for top-ranked |
| Rank badge bg | `linear-gradient(135deg, rgba(217,119,6,0.6), rgba(251,191,36,0.4))` | Gold gradient |
| Rank badge bg (even) | `linear-gradient(135deg, rgba(56,189,248,0.5), rgba(125,211,252,0.3))` | Blue gradient |
| Art zone bg | `rgba(14, 12, 10, 0.5)` | Deep black for image area |
| Title text | `#E8E4D9` | Warm off-white |
| Summary text | `rgba(232, 228, 217, 0.7)` | Muted off-white |
| Bullet text | `rgba(186, 230, 253, 0.92)` | Arctic blue |
| Selected glow | `0 0 24px rgba(110, 231, 183, 0.3)` | Emerald halo |
| Hover lift shadow | `0 16px 48px rgba(0, 0, 0, 0.5), 0 0 20px rgba(110, 231, 183, 0.15)` | Deep + glow |

### Rarity System (Mapped to Rank)

Inspired by Slay the Spire (grey/blue/gold banners) and MTG (common/uncommon/rare/mythic):

| Rank | Rarity | Frame Accent | Badge |
|------|--------|-------------|-------|
| 1 | **Legendary** | Gold border + gold glow | Gold pill, larger card |
| 2-3 | **Rare** | Emerald border | Emerald pill |
| 4-5 | **Uncommon** | Blue border | Blue pill |
| 6+ | **Common** | Subtle grey border | Grey pill |

---

## 2. Card Anatomy

Each card in the fan follows a **TCG-inspired layout**. This is NOT a standard UI card — it's designed to look like a **physical game card**.

### Card Structure (225×320px base, ~5:7 ratio)

```
┌─────────────────────────────────┐ ← Outer frame (2px border, rounded 12px)
│ ┌─[1]──────────────────[RANK]─┐ │
│ │                              │ │ ← Rank badge top-right (circle, 28px)
│ │    ┌──────────────────┐     │ │
│ │    │                  │     │ │ ← Art zone (image or icon placeholder)
│ │    │   📷 IMAGE AREA  │     │ │    Height: ~40% of card
│ │    │   (or gradient   │     │ │    Aspect: 16:9 within art frame
│ │    │    placeholder)  │     │ │
│ │    └──────────────────┘     │ │
│ │                              │ │
│ │  ═══ TITLE BAR ════════════ │ │ ← Title: bold, 14-16px, max 2 lines
│ │                              │ │    Divider line below
│ │  Summary line goes here,    │ │ ← Summary: italic, 12-13px, max 2 lines
│ │  truncated if too long...   │ │
│ │                              │ │
│ │  1. First bullet point      │ │ ← Bullets: 11-12px, max 3 shown in card
│ │  2. Second bullet point     │ │    (full list visible on expand)
│ │  3. Third bullet point      │ │
│ │                              │ │
│ └──────────────────────────────┘ │
│ ┌──────────────────────────────┐ │
│ │  ✨ Details available...     │ │ ← Footer hint (if details exist)
│ └──────────────────────────────┘ │
└─────────────────────────────────┘
```

### Art Zone Behavior

- **If `node.image` exists:** Show the image, object-fit cover, 16:9
- **If no image:** Show a **gradient placeholder** using the rank's accent color:
  - Rank 1: Gold radial gradient with noise texture
  - Rank 2-3: Emerald radial gradient
  - Rank 4+: Blue-grey radial gradient
- **Placeholder icon:** A subtle SVG icon in the center (e.g., a brain icon for "cause", lightning for "effect", question mark for "question", etc.) at 20% opacity

### Title Bar

- Font: weight 700, `clamp(13px, 1.2vw, 16px)`
- Color: `#E8E4D9` (off-white)
- Max 2 lines with `-webkit-line-clamp: 2`
- Bottom divider: 1px solid `rgba(255, 255, 255, 0.08)`
- Uses `resolveNodeTitle(node)` — same logic as timeline

### Summary Line

- Font: weight 400, italic, `clamp(11px, 1vw, 13px)`
- Color: `rgba(232, 228, 217, 0.65)`
- Max 2 lines with `-webkit-line-clamp: 2`
- Uses `resolveSummaryLine(node, ...)` — same logic as timeline

### Bullet Points (Compact — max 3 in card view)

- Font: 11-12px, line-height 1.4
- Number color: accent color matching rank rarity
- Text color: `rgba(186, 230, 253, 0.85)` (arctic blue, slightly muted vs timeline)
- Only show first 3 bullets in the card; full list in expanded view
- If more than 3, show "... +N more" text

### Rank Badge

- **Position:** Top-right corner of the inner frame, overlapping the art zone slightly
- **Shape:** Circle, 28px diameter
- **Font:** 14px, weight 800
- **Background:** Matches rarity gradient
- **Border:** 2px solid matching but darker accent
- **Z-index:** Above art zone

---

## 3. Fan Layout Math

### Core Algorithm (Slay the Spire Style)

The fan uses a **normalized position system** where each card gets a value `t` from -1 (leftmost) to +1 (rightmost).

```typescript
// === FAN LAYOUT CONSTANTS ===
const FAN_CONFIG = {
  /** Max rotation at the edges of the fan (degrees) */
  ROT_MAX: 12,
  /** Height of the parabolic arc (px) — how much center card rises */
  ARC_HEIGHT: 40,
  /** Negative overlap between cards (px) — negative = overlap */
  CARD_SPACING: -45,
  /** Base card width (px) */
  CARD_WIDTH: 225,
  /** Base card height (px) */
  CARD_HEIGHT: 320,
  /** Scale multiplier on hover */
  HOVER_SCALE: 1.3,
  /** How far card lifts on hover (px) */
  HOVER_LIFT: -60,
  /** Transform-origin Y for rotation (makes cards rotate from bottom-center like held in hand) */
  ORIGIN_Y: "120%",
  /** Z-index base for stacking */
  Z_BASE: 1,
  /** Maximum cards before we reduce card size */
  MAX_FULL_SIZE: 7,
  /** Minimum card scale when there are many cards */
  MIN_SCALE: 0.7,
};

// === POSITION CALCULATION ===
function computeFanPositions(cardCount: number): FanCardPosition[] {
  const positions: FanCardPosition[] = [];

  for (let i = 0; i < cardCount; i++) {
    // Normalized position: -1 (left) to +1 (right)
    const t = cardCount === 1 ? 0 : (i / (cardCount - 1)) * 2 - 1;

    // Rotation: linear from -ROT_MAX to +ROT_MAX
    const rotation = t * FAN_CONFIG.ROT_MAX;

    // Arc offset: parabolic curve, highest at center (t=0)
    //   y = -ARC_HEIGHT * t² + ARC_HEIGHT
    //   At t=0 (center): y = ARC_HEIGHT (highest)
    //   At t=±1 (edges): y = 0 (lowest)
    const arcOffset = -FAN_CONFIG.ARC_HEIGHT * (t * t) + FAN_CONFIG.ARC_HEIGHT;

    // Horizontal position: centered, with overlap
    const totalWidth = (cardCount - 1) * (FAN_CONFIG.CARD_WIDTH + FAN_CONFIG.CARD_SPACING);
    const startX = -totalWidth / 2;
    const x = startX + i * (FAN_CONFIG.CARD_WIDTH + FAN_CONFIG.CARD_SPACING);

    // Z-index: center cards on top (so they overlap edges)
    const zIndex = FAN_CONFIG.Z_BASE + cardCount - Math.abs(Math.round(t * cardCount));

    // Scale reduction for many cards
    const scale = cardCount > FAN_CONFIG.MAX_FULL_SIZE
      ? Math.max(FAN_CONFIG.MIN_SCALE, 1 - (cardCount - FAN_CONFIG.MAX_FULL_SIZE) * 0.05)
      : 1;

    positions.push({
      index: i,
      t,
      x,
      y: -arcOffset, // Negative because CSS translateY up is negative
      rotation,
      zIndex,
      scale,
    });
  }

  return positions;
}

type FanCardPosition = {
  index: number;
  t: number;        // Normalized position (-1 to 1)
  x: number;        // Horizontal offset from center (px)
  y: number;        // Vertical offset (px, negative = up)
  rotation: number;  // Degrees
  zIndex: number;
  scale: number;
};
```

### Hover Behavior

When a card is hovered:
1. **Lift:** `translateY(HOVER_LIFT)` — card rises above the fan
2. **Un-rotate:** `rotate(0deg)` — card straightens to vertical
3. **Scale:** `scale(HOVER_SCALE)` — card enlarges 1.3x
4. **Z-index:** Jump to `100` — always on top
5. **Shadow:** Enhanced drop shadow + glow
6. **Adjacent cards:** Push apart slightly (CSS `~ .sibling` selector shifts ±15px)
7. **Transition:** `transform 200ms cubic-bezier(0.34, 1.56, 0.64, 1)` — slight overshoot for game feel

### Transform-Origin

**Critical:** Set `transform-origin: center 120%` (or `center bottom`). This makes cards rotate from a point **below the card** (as if held in a hand), creating the natural fan arc. Without this, cards rotate from their center and look wrong.

### iPad/Touch Adaptation

- **No hover on touch** — tap to select instead
- On tap: card lifts + expands inline (same as hover visual)
- Swipe left/right through the fan? (stretch goal)
- Reduce `ROT_MAX` to 8 and `ARC_HEIGHT` to 25 on screens < 720px

---

## 4. Interaction Model

### Desktop (Mouse + Keyboard)

| Action | Behavior |
|--------|----------|
| **Hover card** | Lift + scale + un-rotate + show full content. Adjacent cards push apart. |
| **Click card** | Toggle **selected** state. Selected card stays lifted. Emits node selection. |
| **Right-click card** | Open context menu (same restructure actions as timeline) |
| **Press `1-9`** | Select card by rank number. Scrolls fan to center on that card if needed. |
| **Press `Escape`** | Deselect card, close context menu |
| **Press `←` `→`** | Navigate between cards in the fan (move selection left/right) |

### iPad (Touch)

| Action | Behavior |
|--------|----------|
| **Tap card** | Select + lift (equivalent to click) |
| **Long-press card** | Open context menu |
| **Tap elsewhere** | Deselect |

### Detail Expansion (Selected Card)

When a card is **selected** (clicked, not just hovered), show an **expanded detail panel** above the fan:

```
┌──────────────────────────────────────────────────┐
│                                                  │
│  ┌──────────────────────────────────────────┐   │
│  │  EXPANDED DETAIL PANEL                    │   │
│  │                                           │   │
│  │  Title (large, 20-24px)                   │   │
│  │  Summary line (full, not truncated)       │   │
│  │                                           │   │
│  │  1. Full bullet point text here           │   │
│  │  2. Another bullet point                  │   │
│  │  3. Third bullet point                    │   │
│  │  4. Fourth bullet (was hidden in card)    │   │
│  │  5. Fifth bullet                          │   │
│  │                                           │   │
│  │  📖 Details paragraph if available...     │   │
│  │                                           │   │
│  │  📷 Full-size image (if available)        │   │
│  └──────────────────────────────────────────┘   │
│                                                  │
│          ╭────╮ ╭────╮ ╭────╮ ╭────╮            │
│         │    ││ ◆◆ ││    ││    │            │
│          ╰────╯ ╰────╯ ╰────╯ ╰────╯            │
│                  (fan, selected card glows)       │
└──────────────────────────────────────────────────┘
```

The detail panel should:
- Animate in from below (slide up 16px + fade in)
- Have the same glassy background as the statement card
- Show ALL bullets (not just first 3)
- Show full details text if available
- Show full-size image if available
- Dismiss when pressing Escape or clicking another card

---

## 5. Architecture & Files

### Files to Create

| File | Purpose |
|------|---------|
| `src/features/life-stream/components/stream/CardFanLayout.tsx` | New component: fan layout container + card positioning |
| `src/features/life-stream/components/stream/CardFanLayout.css` | All fan-specific styles |
| `src/features/life-stream/components/stream/FanCard.tsx` | Individual card in the fan (TCG-style card rendering) |
| `src/features/life-stream/components/stream/FanCard.css` | Card frame + art zone + title/summary/bullet styles |

### Files to Modify

| File | Changes |
|------|---------|
| `CauseEffectCard.tsx` | Add `layoutMode` toggle, conditionally render `CardFanLayout` vs timeline |
| `CauseEffectCard.css` | Add `.life-causal-card--fan` modifier, fan container styles |
| `CardBubble.tsx` | Pass `layoutMode` prop (or read from a toggle state) |

### Files to Reference (No Changes)

| File | Why |
|------|-----|
| `types.ts` | `CausalNode` shape — FanCard reads same data |
| `CauseEffectCard.tsx` (utility fns) | Reuse `resolveNodeTitle`, `resolveSummaryLine`, `resolveRightNodeBullets`, `resolveImageSrc`, `nodeAnchorId` |
| `LifeMessageRow.css` | Visual consistency reference |

### Component Tree

```
CauseEffectCard (existing)
├── layoutMode === "timeline" → TimelineSection[] (existing Spine Flow)
├── layoutMode === "fan" → CardFanLayout (NEW)
│   ├── FanCard[] (one per rightNode)
│   │   ├── .fan-card__frame
│   │   ├── .fan-card__rank-badge
│   │   ├── .fan-card__art-zone (image or gradient placeholder)
│   │   ├── .fan-card__title-bar
│   │   ├── .fan-card__summary
│   │   ├── .fan-card__bullets (max 3)
│   │   └── .fan-card__footer
│   └── FanDetailPanel (expanded view for selected card)
└── NodeCard[] (horizontal mode, unchanged)
```

---

## 6. Step-by-Step Implementation

### Step 1: Create `FanCard.tsx` + `FanCard.css`

**This is the individual card component.** It renders a single CausalNode as a TCG-style card.

#### FanCard Props

```typescript
type FanCardProps = {
  node: CausalNode;
  index: number;
  rank: number;
  bullets: string[];
  isSelected: boolean;
  isHovered: boolean;
  /** Pre-computed fan position */
  position: FanCardPosition;
  /** Rarity tier derived from rank */
  rarity: "legendary" | "rare" | "uncommon" | "common";
  onSelect: (nodeId: string) => void;
  onHover: (nodeId: string | null) => void;
  onContextMenu: (e: React.MouseEvent | React.TouchEvent, nodeId: string) => void;
};
```

#### FanCard JSX Structure

```tsx
<div
  className={cx("fan-card", {
    "fan-card--legendary": rarity === "legendary",
    "fan-card--rare": rarity === "rare",
    "fan-card--uncommon": rarity === "uncommon",
    "fan-card--common": rarity === "common",
    "fan-card--selected": isSelected,
    "fan-card--hovered": isHovered,
  })}
  style={{
    "--fan-x": `${position.x}px`,
    "--fan-y": `${position.y}px`,
    "--fan-rot": `${position.rotation}deg`,
    "--fan-z": position.zIndex,
    "--fan-scale": position.scale,
  } as React.CSSProperties}
  id={nodeAnchorId(node.id)}
  onClick={() => onSelect(node.id)}
  onMouseEnter={() => onHover(node.id)}
  onMouseLeave={() => onHover(null)}
  onContextMenu={(e) => { e.preventDefault(); onContextMenu(e, node.id); }}
  tabIndex={0}
  role="button"
  aria-label={`Card ${rank}: ${nodeTitle}`}
>
  {/* Inner frame */}
  <div className="fan-card__inner">
    {/* Rank badge */}
    <div className="fan-card__rank">{rank}</div>

    {/* Art zone */}
    <div className="fan-card__art">
      {imageSrc ? (
        <img src={resolveImageSrc(imageSrc)} alt="" className="fan-card__art-img" />
      ) : (
        <div className="fan-card__art-placeholder">
          {/* Gradient placeholder with semantic icon */}
          <svg className="fan-card__art-icon" aria-hidden>
            {/* Icon based on semantic mode */}
          </svg>
        </div>
      )}
    </div>

    {/* Title */}
    <h4 className="fan-card__title">{nodeTitle}</h4>

    {/* Divider */}
    <div className="fan-card__divider" />

    {/* Summary */}
    {summaryLine && (
      <p className="fan-card__summary">{summaryLine}</p>
    )}

    {/* Bullets (max 3 in card view) */}
    {bullets.length > 0 && (
      <ul className="fan-card__bullets">
        {bullets.slice(0, 3).map((b, i) => (
          <li key={i}>
            <span className="fan-card__bullet-num">{i + 1}</span>
            <span className="fan-card__bullet-text">{b}</span>
          </li>
        ))}
        {bullets.length > 3 && (
          <li className="fan-card__bullets-more">
            +{bullets.length - 3} more...
          </li>
        )}
      </ul>
    )}
  </div>

  {/* Footer (detail hint) */}
  {node.details && (
    <div className="fan-card__footer">
      <span>Click to expand</span>
    </div>
  )}
</div>
```

#### FanCard.css Key Styles

```css
/* === BASE CARD === */
.fan-card {
  position: absolute;
  width: 225px;
  height: 320px;
  /* Position from CSS variables set by JS */
  left: 50%;
  top: 0;
  transform:
    translateX(calc(-50% + var(--fan-x, 0px)))
    translateY(var(--fan-y, 0px))
    rotate(var(--fan-rot, 0deg))
    scale(var(--fan-scale, 1));
  transform-origin: center 120%;
  z-index: var(--fan-z, 1);
  /* Transition for smooth fan layout */
  transition:
    transform 200ms cubic-bezier(0.34, 1.56, 0.64, 1),
    box-shadow 200ms ease,
    z-index 0ms;
  cursor: pointer;
  user-select: none;
  -webkit-user-select: none;
  /* Avoid blurry text during transforms */
  will-change: transform;
  backface-visibility: hidden;
}

/* === INNER FRAME (the actual card face) === */
.fan-card__inner {
  width: 100%;
  height: 100%;
  background: rgba(28, 26, 22, 0.88);
  border: 2px solid rgba(110, 231, 183, 0.2);
  border-radius: 12px;
  overflow: hidden;
  padding: 8px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  /* Glass effect */
  backdrop-filter: blur(12px) saturate(1.2);
  /* Inner glow */
  box-shadow:
    inset 0 1px 0 rgba(255, 250, 230, 0.1),
    0 4px 16px rgba(0, 0, 0, 0.35);
  /* Noise texture (reuse existing) */
  position: relative;
}

/* Noise texture overlay (same as existing cards) */
.fan-card__inner::after {
  content: "";
  position: absolute;
  inset: 0;
  background: url("data:image/svg+xml,...") repeat; /* Same noise as CauseEffectCard */
  opacity: 0.04;
  pointer-events: none;
  border-radius: inherit;
}

/* === HOVER STATE === */
.fan-card--hovered:not(.fan-card--selected) {
  transform:
    translateX(calc(-50% + var(--fan-x, 0px)))
    translateY(-60px)
    rotate(0deg)
    scale(1.3);
  z-index: 100 !important;
  box-shadow:
    0 16px 48px rgba(0, 0, 0, 0.5),
    0 0 20px rgba(110, 231, 183, 0.15);
}

/* Adjacent cards push apart on hover */
.fan-card--hovered ~ .fan-card {
  transform:
    translateX(calc(-50% + var(--fan-x, 0px) + 15px))
    translateY(var(--fan-y, 0px))
    rotate(var(--fan-rot, 0deg))
    scale(var(--fan-scale, 1));
}

/* === SELECTED STATE === */
.fan-card--selected {
  transform:
    translateX(calc(-50% + var(--fan-x, 0px)))
    translateY(-60px)
    rotate(0deg)
    scale(1.3);
  z-index: 100 !important;
}
.fan-card--selected .fan-card__inner {
  border-color: rgba(110, 231, 183, 0.5);
  box-shadow:
    inset 0 1px 0 rgba(255, 250, 230, 0.15),
    0 0 24px rgba(110, 231, 183, 0.3),
    0 16px 48px rgba(0, 0, 0, 0.5);
}

/* === RARITY VARIANTS === */
.fan-card--legendary .fan-card__inner {
  border-color: rgba(251, 191, 36, 0.45);
}
.fan-card--legendary .fan-card__rank {
  background: linear-gradient(135deg, rgba(217, 119, 6, 0.7), rgba(251, 191, 36, 0.5));
  box-shadow: 0 0 10px rgba(251, 191, 36, 0.3);
}

.fan-card--rare .fan-card__inner {
  border-color: rgba(110, 231, 183, 0.3);
}
.fan-card--rare .fan-card__rank {
  background: linear-gradient(135deg, rgba(52, 211, 153, 0.6), rgba(110, 231, 183, 0.4));
}

.fan-card--uncommon .fan-card__inner {
  border-color: rgba(125, 211, 252, 0.25);
}
.fan-card--uncommon .fan-card__rank {
  background: linear-gradient(135deg, rgba(56, 189, 248, 0.5), rgba(125, 211, 252, 0.3));
}

.fan-card--common .fan-card__inner {
  border-color: rgba(255, 255, 255, 0.1);
}
.fan-card--common .fan-card__rank {
  background: rgba(120, 113, 108, 0.5);
}

/* === RANK BADGE === */
.fan-card__rank {
  position: absolute;
  top: 6px;
  right: 6px;
  width: 28px;
  height: 28px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 14px;
  font-weight: 800;
  color: #fff;
  z-index: 2;
  border: 2px solid rgba(14, 12, 10, 0.8);
  text-shadow: 0 1px 2px rgba(0, 0, 0, 0.5);
}

/* === ART ZONE === */
.fan-card__art {
  width: 100%;
  height: 40%;
  border-radius: 8px;
  overflow: hidden;
  position: relative;
  flex-shrink: 0;
}

.fan-card__art-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.fan-card__art-placeholder {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
}
/* Legendary placeholder */
.fan-card--legendary .fan-card__art-placeholder {
  background: radial-gradient(ellipse at 30% 40%, rgba(251, 191, 36, 0.15), transparent 70%),
              radial-gradient(ellipse at 70% 60%, rgba(217, 119, 6, 0.1), transparent 60%),
              rgba(14, 12, 10, 0.5);
}
/* Rare placeholder */
.fan-card--rare .fan-card__art-placeholder {
  background: radial-gradient(ellipse at 40% 40%, rgba(110, 231, 183, 0.12), transparent 70%),
              rgba(14, 12, 10, 0.5);
}
/* Uncommon placeholder */
.fan-card--uncommon .fan-card__art-placeholder {
  background: radial-gradient(ellipse at 50% 50%, rgba(125, 211, 252, 0.1), transparent 70%),
              rgba(14, 12, 10, 0.5);
}
/* Common placeholder */
.fan-card--common .fan-card__art-placeholder {
  background: rgba(14, 12, 10, 0.5);
}

.fan-card__art-icon {
  width: 40px;
  height: 40px;
  opacity: 0.15;
  fill: currentColor;
  color: rgba(232, 228, 217, 0.5);
}

/* === TITLE === */
.fan-card__title {
  font-size: clamp(13px, 1.2vw, 16px);
  font-weight: 700;
  color: #E8E4D9;
  margin: 0;
  line-height: 1.3;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

/* === DIVIDER === */
.fan-card__divider {
  height: 1px;
  background: rgba(255, 255, 255, 0.08);
  flex-shrink: 0;
}

/* === SUMMARY === */
.fan-card__summary {
  font-size: clamp(11px, 0.9vw, 13px);
  font-weight: 400;
  font-style: italic;
  color: rgba(232, 228, 217, 0.65);
  margin: 0;
  line-height: 1.35;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

/* === BULLETS === */
.fan-card__bullets {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
  flex: 1;
  min-height: 0;
  overflow: hidden;
}

.fan-card__bullets li {
  display: flex;
  gap: 6px;
  align-items: baseline;
  font-size: 11px;
  line-height: 1.4;
}

.fan-card__bullet-num {
  font-weight: 700;
  font-size: 10px;
  min-width: 14px;
  flex-shrink: 0;
}
/* Rarity-colored bullet numbers */
.fan-card--legendary .fan-card__bullet-num { color: rgba(251, 191, 36, 0.8); }
.fan-card--rare .fan-card__bullet-num { color: rgba(110, 231, 183, 0.8); }
.fan-card--uncommon .fan-card__bullet-num { color: rgba(125, 211, 252, 0.8); }
.fan-card--common .fan-card__bullet-num { color: rgba(200, 200, 200, 0.6); }

.fan-card__bullet-text {
  color: rgba(186, 230, 253, 0.85);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.fan-card__bullets-more {
  font-size: 10px;
  color: rgba(232, 228, 217, 0.4);
  font-style: italic;
}

/* === FOOTER === */
.fan-card__footer {
  font-size: 10px;
  color: rgba(232, 228, 217, 0.35);
  text-align: center;
  padding: 4px;
  border-top: 1px solid rgba(255, 255, 255, 0.05);
}

/* === RESPONSIVE (iPad / mobile) === */
@media (max-width: 720px) {
  .fan-card {
    width: 180px;
    height: 256px;
  }
  .fan-card__title { font-size: 12px; }
  .fan-card__summary { font-size: 10px; }
  .fan-card__bullets li { font-size: 10px; }
  .fan-card__rank { width: 24px; height: 24px; font-size: 12px; }
}
```

---

### Step 2: Create `CardFanLayout.tsx` + `CardFanLayout.css`

**This is the container that positions all FanCards in the arc.**

#### CardFanLayout Props

```typescript
type CardFanLayoutProps = {
  cardId: string;
  rightNodes: CausalNode[];
  rightNodeBulletsMap: Map<string, string[]>;
  selectedNodeId: string | null;
  onSelectNode: (nodeId: string | null) => void;
  onContextMenu: (e: React.MouseEvent | React.TouchEvent, nodeId: string) => void;
  semanticMode: CausalSemanticMode;
};
```

#### CardFanLayout JSX Structure

```tsx
function CardFanLayout({
  rightNodes,
  rightNodeBulletsMap,
  selectedNodeId,
  onSelectNode,
  onContextMenu,
}: CardFanLayoutProps) {
  const [hoveredNodeId, setHoveredNodeId] = useState<string | null>(null);

  // Compute fan positions
  const positions = useMemo(
    () => computeFanPositions(rightNodes.length),
    [rightNodes.length]
  );

  // Derive rarity from rank
  const getRarity = (rank: number): FanCardRarity => {
    if (rank === 1) return "legendary";
    if (rank <= 3) return "rare";
    if (rank <= 5) return "uncommon";
    return "common";
  };

  // Selected node details for expanded panel
  const selectedNode = selectedNodeId
    ? rightNodes.find((n) => n.id === selectedNodeId)
    : null;

  return (
    <div className="card-fan-layout">
      {/* Expanded detail panel (above the fan) */}
      {selectedNode && (
        <FanDetailPanel
          node={selectedNode}
          bullets={rightNodeBulletsMap.get(selectedNode.id) ?? []}
          onClose={() => onSelectNode(null)}
        />
      )}

      {/* Fan container */}
      <div
        className="card-fan-layout__fan"
        style={{
          // Container height = card height + arc height + hover lift space
          height: `${FAN_CONFIG.CARD_HEIGHT + FAN_CONFIG.ARC_HEIGHT + 80}px`,
        }}
      >
        {rightNodes.map((node, i) => (
          <FanCard
            key={node.id}
            node={node}
            index={i}
            rank={node.rank ?? i + 1}
            bullets={rightNodeBulletsMap.get(node.id) ?? []}
            isSelected={node.id === selectedNodeId}
            isHovered={node.id === hoveredNodeId}
            position={positions[i]}
            rarity={getRarity(node.rank ?? i + 1)}
            onSelect={onSelectNode}
            onHover={setHoveredNodeId}
            onContextMenu={onContextMenu}
          />
        ))}
      </div>
    </div>
  );
}
```

#### FanDetailPanel (Sub-Component)

```tsx
/** Expanded view shown above the fan when a card is selected */
function FanDetailPanel({
  node,
  bullets,
  onClose,
}: {
  node: CausalNode;
  bullets: string[];
  onClose: () => void;
}) {
  const title = resolveNodeTitle(node);
  const summary = resolveSummaryLine(node);
  const imageSrc = node.image?.url ? resolveImageSrc(node.image.url) : null;

  return (
    <div className="fan-detail-panel" role="region" aria-label="Card details">
      <button className="fan-detail-panel__close" onClick={onClose} aria-label="Close">
        ✕
      </button>

      {imageSrc && (
        <img src={imageSrc} alt="" className="fan-detail-panel__image" />
      )}

      <h3 className="fan-detail-panel__title">{title}</h3>

      {summary && (
        <p className="fan-detail-panel__summary">{summary}</p>
      )}

      {bullets.length > 0 && (
        <ol className="fan-detail-panel__bullets">
          {bullets.map((b, i) => (
            <li key={i}>{b}</li>
          ))}
        </ol>
      )}

      {node.details && (
        <div className="fan-detail-panel__details">{node.details}</div>
      )}
    </div>
  );
}
```

#### CardFanLayout.css

```css
.card-fan-layout {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 24px;
  width: 100%;
}

/* === FAN CONTAINER === */
.card-fan-layout__fan {
  position: relative;
  width: 100%;
  min-width: 400px;
  /* Height set by inline style based on card dimensions */
}

/* === DETAIL PANEL (expanded card view) === */
.fan-detail-panel {
  width: 100%;
  max-width: 600px;
  background: rgba(28, 26, 22, 0.75);
  backdrop-filter: blur(16px) saturate(1.3);
  border: 1px solid rgba(110, 231, 183, 0.2);
  border-radius: 14px;
  padding: 20px 24px;
  position: relative;
  /* Animate in */
  animation: fanDetailSlideIn 250ms ease-out;
  box-shadow:
    inset 0 1px 0 rgba(255, 250, 230, 0.12),
    0 8px 32px rgba(0, 0, 0, 0.45),
    0 0 16px rgba(110, 231, 183, 0.08);
}

@keyframes fanDetailSlideIn {
  from {
    opacity: 0;
    transform: translateY(12px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

.fan-detail-panel__close {
  position: absolute;
  top: 12px;
  right: 14px;
  background: none;
  border: none;
  color: rgba(232, 228, 217, 0.5);
  font-size: 16px;
  cursor: pointer;
  padding: 4px 8px;
  border-radius: 6px;
  transition: color 120ms, background 120ms;
}
.fan-detail-panel__close:hover {
  color: #E8E4D9;
  background: rgba(255, 255, 255, 0.06);
}

.fan-detail-panel__image {
  width: 100%;
  max-height: 200px;
  object-fit: cover;
  border-radius: 10px;
  margin-bottom: 14px;
}

.fan-detail-panel__title {
  font-size: clamp(18px, 1.6vw, 24px);
  font-weight: 760;
  color: rgba(110, 231, 183, 0.95);
  margin: 0 0 8px;
  line-height: 1.25;
}

.fan-detail-panel__summary {
  font-size: clamp(14px, 1.1vw, 16px);
  font-style: italic;
  color: rgba(232, 228, 217, 0.7);
  margin: 0 0 12px;
  line-height: 1.4;
}

.fan-detail-panel__bullets {
  margin: 0 0 12px;
  padding-left: 20px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.fan-detail-panel__bullets li {
  font-size: 15px;
  line-height: 1.48;
  color: rgba(186, 230, 253, 0.92);
}
.fan-detail-panel__bullets li::marker {
  color: rgba(110, 231, 183, 0.6);
}

.fan-detail-panel__details {
  font-size: 14px;
  color: rgba(232, 228, 217, 0.6);
  line-height: 1.5;
  border-top: 1px solid rgba(255, 255, 255, 0.06);
  padding-top: 10px;
  margin-top: 4px;
}

/* === RESPONSIVE === */
@media (max-width: 720px) {
  .card-fan-layout__fan {
    min-width: unset;
    overflow-x: auto;
    -webkit-overflow-scrolling: touch;
  }
  .fan-detail-panel {
    padding: 14px 16px;
  }
  .fan-detail-panel__title { font-size: 18px; }
}
```

---

### Step 3: Integrate into CauseEffectCard.tsx

#### Add Layout Mode State

In `CauseEffectCard.tsx`, add a layout mode toggle:

```typescript
// At the top of the component:
type RightLaneMode = "timeline" | "fan";

// Inside the component:
const [rightLaneMode, setRightLaneMode] = useState<RightLaneMode>("timeline");
```

#### Add Mode Toggle Button

In the right lane label area (where "💡 WHY" is), add a toggle:

```tsx
<div className="life-causal-card__lane-label life-causal-card__lane-label--right">
  {rightLaneIcon} {rightLaneLabel}
  {/* Layout mode toggle */}
  <button
    className="life-causal-card__layout-toggle"
    onClick={() => setRightLaneMode(m => m === "timeline" ? "fan" : "timeline")}
    title={rightLaneMode === "timeline" ? "Switch to card fan view" : "Switch to timeline view"}
    aria-label="Toggle layout mode"
  >
    {rightLaneMode === "timeline" ? "🃏" : "📋"}
  </button>
</div>
```

#### Conditional Rendering

In the vertical layout branch for the right lane:

```tsx
{/* Right lane */}
<div className={`life-causal-card__lane life-causal-card__lane--right ${
  rightLaneMode === "fan" ? "life-causal-card__lane--fan" : "life-causal-card__lane--timeline"
}`}>
  {rightLaneMode === "fan" ? (
    <CardFanLayout
      cardId={cardId}
      rightNodes={rankedRightNodes}
      rightNodeBulletsMap={rightNodeBulletsMap}
      selectedNodeId={selectedNodeId}
      onSelectNode={(id) => {
        setSelectedNodeId(prev => prev === id ? null : id);
      }}
      onContextMenu={(e, nodeId) => {
        handleOpenContextMenu(e as React.MouseEvent, nodeId);
      }}
      semanticMode={semanticMode}
    />
  ) : (
    /* Existing TimelineSection rendering */
    rankedRightNodes.map((node, idx) => (
      <TimelineSection key={node.id} node={node} index={idx} ... />
    ))
  )}
</div>
```

#### CSS for Toggle

```css
.life-causal-card__layout-toggle {
  background: none;
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 6px;
  padding: 2px 8px;
  font-size: 14px;
  cursor: pointer;
  margin-left: 8px;
  color: rgba(232, 228, 217, 0.5);
  transition: border-color 150ms, color 150ms, background 150ms;
  vertical-align: middle;
}
.life-causal-card__layout-toggle:hover {
  border-color: rgba(110, 231, 183, 0.3);
  color: rgba(232, 228, 217, 0.8);
  background: rgba(110, 231, 183, 0.05);
}

/* Fan mode: remove spine line and timeline padding */
.life-causal-card__lane--fan {
  padding-left: 0 !important;
}
.life-causal-card__lane--fan::before {
  display: none !important;
}
```

---

### Step 4: Keyboard Handler Updates

Add arrow key navigation for fan mode:

```typescript
// In the existing keyboard handler (useEffect with keydown):
if (rightLaneMode === "fan") {
  if (key === "ArrowLeft" || key === "ArrowRight") {
    e.preventDefault();
    const currentIdx = selectedNodeId
      ? rankedRightNodes.findIndex((n) => n.id === selectedNodeId)
      : -1;
    const nextIdx = key === "ArrowLeft"
      ? Math.max(0, currentIdx - 1)
      : Math.min(rankedRightNodes.length - 1, currentIdx + 1);
    setSelectedNodeId(rankedRightNodes[nextIdx]?.id ?? null);
  }
}
// Existing 1-9 handler works for both modes (selects by rank)
```

---

### Step 5: Long-Press Support for FanCard

Reuse the same long-press pattern from TimelineSection/NodeCard:

```typescript
// In FanCard, add:
const longPressTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

const handlePointerDown = (e: React.PointerEvent) => {
  if (e.button !== 0) return;
  longPressTimer.current = setTimeout(() => {
    onSelect(node.id);
    onContextMenu(e as unknown as React.MouseEvent, node.id);
  }, 320);
};

const handlePointerUp = () => {
  if (longPressTimer.current) {
    clearTimeout(longPressTimer.current);
    longPressTimer.current = null;
  }
};
```

---

### Step 6: Responsive Adjustments

The fan needs special handling for narrow viewports:

```css
/* Small screens: horizontal scroll instead of fitting all cards */
@media (max-width: 720px) {
  .card-fan-layout__fan {
    overflow-x: auto;
    overflow-y: visible;
    -webkit-overflow-scrolling: touch;
    padding: 20px 0;
  }

  /* Reduce fan constants via CSS overrides */
  .fan-card {
    width: 180px;
    height: 256px;
  }
}
```

In the JS, adapt fan constants for mobile:

```typescript
const isMobile = window.innerWidth <= 720;
const config = isMobile
  ? { ...FAN_CONFIG, ROT_MAX: 8, ARC_HEIGHT: 25, CARD_WIDTH: 180, CARD_HEIGHT: 256 }
  : FAN_CONFIG;
```

---

## 7. CSS Reference

### Full CSS Custom Properties

```css
:root {
  /* Card dimensions */
  --fan-card-w: 225px;
  --fan-card-h: 320px;
  --fan-card-radius: 12px;

  /* Colors */
  --fan-frame-bg: rgba(28, 26, 22, 0.88);
  --fan-frame-border: rgba(110, 231, 183, 0.2);
  --fan-title-color: #E8E4D9;
  --fan-summary-color: rgba(232, 228, 217, 0.65);
  --fan-bullet-color: rgba(186, 230, 253, 0.85);

  /* Rarity colors */
  --fan-legendary-accent: rgba(251, 191, 36, 0.45);
  --fan-rare-accent: rgba(110, 231, 183, 0.3);
  --fan-uncommon-accent: rgba(125, 211, 252, 0.25);
  --fan-common-accent: rgba(255, 255, 255, 0.1);

  /* Animation */
  --fan-transition: 200ms cubic-bezier(0.34, 1.56, 0.64, 1);
  --fan-hover-lift: -60px;
  --fan-hover-scale: 1.3;
}
```

---

## 8. AI Implementor Notes

### Critical Things to Get Right

1. **`transform-origin: center 120%`** on each card — this is what makes the fan look like cards held in a hand. Without it, rotations look mechanical and wrong. The origin point is BELOW the card.

2. **Z-index management** — Center cards must have higher z-index than edge cards so they overlap correctly. On hover, z-index must jump to 100+ immediately (no transition on z-index).

3. **`will-change: transform`** on cards — Promotes to GPU layer for smooth transforms. Also add `backface-visibility: hidden` to prevent subpixel rendering artifacts.

4. **The hover transition curve** — `cubic-bezier(0.34, 1.56, 0.64, 1)` gives a slight overshoot that feels "game-like". A linear or ease transition will feel flat and boring.

5. **Don't use Framer Motion** — The existing codebase does NOT use Framer Motion. Use CSS transitions for all animation. The CSS approach is simpler, more performant, and consistent with the rest of the app.

6. **Card text must be readable at the default size (225×320px)** — This is about the size of a real playing card on screen. Title should be 13-16px, summary 11-13px, bullets 11px. Test that you can actually read the text.

7. **Reuse existing utility functions** — `resolveNodeTitle`, `resolveSummaryLine`, `resolveRightNodeBullets`, `resolveImageSrc`, `nodeAnchorId`. Do NOT rewrite these.

8. **The adjacent card push** — When hovering a card, sibling cards should shift slightly. Use the CSS `~` (general sibling) selector. BUT: this only works for cards AFTER the hovered card in DOM order. For cards BEFORE the hovered card, you may need a JS class like `fan-card--before-hovered` or just accept the asymmetry (Slay the Spire also only pushes cards to the right).

9. **The detail panel is ABOVE the fan, not inline** — Don't try to expand the card in-place (it would break the fan layout). The detail panel is a separate element above the fan container.

10. **iPad: no hover state** — Use `@media (hover: hover)` to only apply hover effects on devices that support it. On touch devices, tap = select directly.

### Things NOT to Do

- **Don't add 3D perspective tilt on mouse move** — Save this for v3. It's complex and not essential.
- **Don't add holographic/foil effects** — Save for v3. The basic card frame is enough for now.
- **Don't add card flip animations** — The cards are always face-up.
- **Don't animate the fan dealing/entry** — Cards should just appear in fan formation. Entry animation is a stretch goal.
- **Don't modify NodeCard.tsx** — The existing NodeCard is for horizontal mode only. FanCard is separate.
- **Don't modify the statement card / left lane** — It stays exactly as it is (glassy card).
- **Don't remove the timeline mode** — Both modes coexist, toggled by a button.

### Utility Function Imports

The utility functions are defined inside `CauseEffectCard.tsx` (not exported). You have two options:

**Option A (Preferred):** Extract `resolveNodeTitle`, `resolveSummaryLine`, `resolveRightNodeBullets`, `resolveImageSrc` into a new file `src/features/life-stream/components/stream/causalNodeUtils.ts` and import from both CauseEffectCard.tsx and FanCard.tsx.

**Option B:** Pass pre-resolved data as props (title string, summary string, bullets array) from CardFanLayout → FanCard, computing them in the parent using the existing utility functions.

**Go with Option B** — it's less refactoring and keeps the utility functions where they are. CardFanLayout already receives `rightNodeBulletsMap`. Compute titles and summaries in CardFanLayout and pass them as props.

### Performance Considerations

- The fan will have at most ~7-9 cards (typical causal graph has 3-6 right nodes). This is not a performance concern.
- CSS transforms with `will-change: transform` are GPU-accelerated. No jank expected.
- The detail panel should use `content-visibility: auto` if the content is long.
- Do NOT use `position: fixed` for the detail panel — it should scroll with the card.

---

## 9. Verification Checklist

| # | Check | How to Verify |
|---|-------|---------------|
| 1 | **Build passes** | `just app` completes without errors |
| 2 | **Toggle button visible** | Next to "💡 WHY" label, clicking switches between 📋/🃏 |
| 3 | **Fan layout renders** | Cards appear in arc formation, center card highest |
| 4 | **Card anatomy correct** | Each card shows: rank badge, art zone (or placeholder), title, summary, bullets |
| 5 | **Rarity colors** | Rank 1 = gold frame, 2-3 = green, 4-5 = blue, 6+ = grey |
| 6 | **Hover effect** | Card lifts, un-rotates, scales 1.3x, shows enhanced shadow |
| 7 | **Click selects** | Clicking a card highlights it and shows detail panel above fan |
| 8 | **Detail panel** | Shows full title, summary, ALL bullets, details text, image |
| 9 | **Right-click context menu** | Same restructure actions work as in timeline mode |
| 10 | **Keyboard 1-9** | Selects card by rank |
| 11 | **Keyboard ← →** | Navigates between cards in fan |
| 12 | **Keyboard Escape** | Deselects and closes detail panel |
| 13 | **Long-press (iPad)** | Opens context menu after 320ms |
| 14 | **No hover on touch** | iPad doesn't show hover effects, only tap-to-select |
| 15 | **Timeline mode preserved** | Toggling back to 📋 shows existing Spine Flow timeline |
| 16 | **Statement card unchanged** | Left lane / parent card looks exactly the same |
| 17 | **3-6 card fan** | Typical card counts look good, no excessive overlap |
| 18 | **1 card edge case** | Single card renders centered, no rotation |
| 19 | **7+ cards** | Cards scale down slightly, still readable |
| 20 | **Responsive (iPad)** | Cards smaller at 720px, horizontally scrollable if needed |

---

## References

- [Slay the Spire Card Hand Demo (Godot)](https://github.com/stormtoy/card_fan_demo) — Fan arc algorithm
- [Hearthstone Card Hover (CodePen)](https://codepen.io/jackrugile/pen/WZGeGM) — CSS hover lift + scale
- [Pokemon Cards CSS Holo Effect](https://github.com/simeydotme/pokemon-cards-css) — Advanced card effects (v3 reference)
- [MTG Card Frame Wiki](https://mtg.fandom.com/wiki/Card_frame) — Frame anatomy + rarity colors
- [CSS 3D Card Fan (WeAreDevelopers)](https://www.wearedevelopers.com/en/magazine/656/creating-a-3d-card-fan-with-css-transforms-656) — CSS transform-origin fan technique
- [Card Game UI Best Practices (Gunslinger's Revenge)](https://www.gunslingersrevenge.com/posts/development/deckbuilder-ui-design-best-practices.html) — Hand layout patterns
