# Card Fan Layout — V2 Plan (High Risk / High Reward)

**Status:** Future implementation (after Spine Flow v1 is stable)
**Inspiration:** Slay the Spire / Monster Train card hand layout

## Concept

Child cards arranged in a fan/arc below the parent card. Each card is slightly rotated based on its position from center. Rank 1 sits at the highest point of the arc (center). Hovering/tapping a card lifts it above the fan, scales it ~1.5x, and shows full detail content.

## Visual Design

- Card rotation: `(index - centerIndex) * angleStep` where angleStep = 3-8 degrees
- Vertical position follows parabolic curve: `y = -k * (index - centerIndex)^2`
- Hover state: `translateY(-20px)` + `scale(1.5)` + enhanced shadow + un-rotate to vertical
- Cards overlap slightly in the hand, creating layered depth
- 3D perspective tilt via CSS `perspective(800px)` + `rotateX/rotateY` on mouse position
- Glare reflection via radial-gradient overlay at cursor position

## Implementation Requirements

- CSS 3D transforms + `perspective` property
- Framer Motion for smooth card lift/rotate/scale animations
- Touch gesture handling for iPad (thumb-arc swipe through fan)
- Detail content zone: either inline expansion above the fan, or a separate "play area" panel
- Card frame styling: mana-cost-style rank badge in top-left, art zone, title, effect text

## Evaluation

| Criterion | Rating |
|-----------|--------|
| Natural flow | Moderate (scannable titles, but detail requires selection) |
| Variable density | Good (uniform in fan, detail on selection only) |
| Desktop + iPad | Good (hover on desktop, tap on iPad, thumb-arc swipe) |
| Scaling | Good up to 7 children (beyond 7, overlap too heavy) |
| Visual distinction | Excellent — unique, game-like, premium feel |

## Key Risks

- Horizontal space requirement (pannable canvas mitigates this)
- Animation complexity (3D transforms + gesture handling)
- Readability at small card sizes in the fan
- Detail content placement needs careful design
- Performance with many cards + 3D transforms

## References

- [Slay the Spire Card Hand Demo (Godot)](https://github.com/stormtoy/card_fan_demo)
- [Generic Card Game UI (Unity)](https://github.com/ycarowr/UiCard)
- [CSS 3D Perspective Animations](https://www.frontend.fyi/tutorials/css-3d-perspective-animations)
- [3D Card Effect Component (Aceternity)](https://ui.aceternity.com/components/3d-card-effect)
- [Interface In Game — Slay the Spire](https://interfaceingame.com/games/slay-the-spire/)
