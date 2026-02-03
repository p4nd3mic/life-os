# TESTING.md — Test Strategy & Patterns

This doc documents the testing approach across the Life OS ecosystem.

> Cross‑refs:
> - **[STATE_MANAGEMENT.md](STATE_MANAGEMENT.md)** for what state should be asserted.
> - **[MCP_INTEGRATION.md](MCP_INTEGRATION.md)** for MCP parsing expectations.
> - **[DATA_MODELS.md](DATA_MODELS.md)** for type/shape contracts that tests should lock down.

---

## Test surface overview

| Project | Framework(s) | Where tests live | Notes |
|---|---|---|---|
| CodexMonitor-lifeos (React/Tauri) | Vitest + React Testing Library | `CodexMonitor-lifeos/src/**/**/*.test.ts(x)` | Node env by default; per-test `@vitest-environment jsdom` where needed |
| CodexMonitor-lifeos (Rust) | `cargo test` | `CodexMonitor-lifeos/src-tauri/src/**` | Small unit tests embedded in modules |
| life-mcp (Node MCP server) | Jest | `life-mcp/__tests__/…` | Unit + integration + schema + regression suites |
| life-os (ops docs) | *(none)* | *(n/a)* | Validate via scripts/linters (future) |
| Obsidian vault | *(none)* | *(n/a)* | Validate via tool outputs + schema tests (future) |

---

## CodexMonitor-lifeos: frontend tests (Vitest)

### Test infrastructure

**Config:** `CodexMonitor-lifeos/vite.config.ts`

Highlights:
- `test.include` targets `src/**/*.test.ts(x)`
- Default environment: `node`
- Setup file: `src/test/vitest.setup.ts`
- **Memory leak prevention**: runs tests in **fork pool** with `maxForks: 1` / `minForks: 1` and `isolate: true`

Why the “single fork” approach?
- This codebase has a lot of globals and long-lived singletons (timers, DOM polyfills, event listeners).
- Parallelism can hide leaks or make failures flaky.
- Isolation + single worker keeps runtime stable on CI and on dev machines.

### Setup file

**File:** `CodexMonitor-lifeos/src/test/vitest.setup.ts`

It does three big things:

1) **Cleanup + reset between tests**
- `@testing-library/react` cleanup
- `vi.clearAllMocks()`
- `vi.clearAllTimers()`

2) **Browser API polyfills**
- `window.matchMedia`
- `ResizeObserver`
- `IntersectionObserver`
- `requestAnimationFrame` / `cancelAnimationFrame`

3) **Local storage shim**
- Adds a minimal in-memory `localStorage` implementation

### Running tests

From `CodexMonitor-lifeos/`:

```bash
npm test            # Single run (vitest run)
npm run test:watch  # Watch mode
npm run test:ui     # Vitest UI
npm run test:coverage
```

> Tip: if you’re debugging a single file, Vitest supports filtering via CLI args:
> `npm test -- src/features/life-stream/components/stream/CardBubble.test.tsx`

---

## Test patterns you should use

### 1) Unit tests for pure utilities

Target: deterministic functions with no async or UI.

Common targets in this repo:
- parsing helpers
- domain/time formatting
- reducer/state helpers
- markdown rendering helpers

Pattern:

```ts
import { describe, it, expect } from "vitest";
import { somePureFn } from "./somePureFn";

describe("somePureFn", () => {
  it("handles the happy path", () => {
    expect(somePureFn("x")).toEqual("y");
  });

  it("handles empty / null inputs", () => {
    expect(somePureFn("")).toEqual(/* … */);
  });
});
```

### 2) Component tests (React Testing Library)

Target: rendering + user interaction (click/typing/keyboard) + conditional UI.

Pattern:

```ts
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

it("submits the composer", async () => {
  const user = userEvent.setup();
  render(<YourComponent />);
  await user.type(screen.getByRole("textbox"), "hello");
  await user.click(screen.getByRole("button", { name: /send/i }));
  expect(/* … */).toBeTruthy();
});
```

### 3) Mocking Tauri commands

Most UI calls into the Rust backend through Tauri `invoke(...)`. In tests, mock the module boundary.

Common pattern:

```ts
import { vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));
```

Then in tests:

```ts
import { invoke } from "@tauri-apps/api/core";

(invoke as unknown as ReturnType<typeof vi.fn>).mockResolvedValue(/* … */);
```

> Some tests require DOM behavior: add `/** @vitest-environment jsdom */` at the top of the test file.

### 4) Mocking MCP tool responses (Life Stream)

There are two layers:

1) **Rust parsing**: tool outputs are often nested, and JSON extraction is best tested close to the parsing layer.
2) **UI/store behavior**: given a parsed card payload, validate card state transitions and rendering.

For frontend tests, prefer mocking **the stream store / adapter** rather than mocking Node itself.

For Rust tests, prefer fixture JSON samples that match:
- “text + JSON blob”
- nested `result.result`
- snake_case vs camelCase mismatches

See: **[GOTCHAS.md](GOTCHAS.md)** for the common pitfalls.

---

## What to test (Life OS critical paths)

### Card state transitions (UI + store)

Focus on:
- draft → pending → confirmed / error
- patch updates merging into existing cards
- dedupe behavior (card IDs, patch versions)

Relevant code:
- `src/features/life-stream/state/streamStore.ts`
- `src/features/life-stream/components/…`

### MCP JSON extraction

Focus on:
- “JSON + text dual format” parsing
- strict vs lenient parsing
- tool-specific extraction (meal/media/expense/delivery/youtube/fitness)

Relevant code:
- `src-tauri/src/life_stream/service.rs`
- `src-tauri/src/life_stream/parsing.rs`
- `src-tauri/src/life_stream/tool_parsers/*.rs`

### Image cache behavior

Focus on:
- TMDB fetch success/fail
- cache key stability
- “already cached” fast path
- local file resolution for UI display

Relevant code:
- `src-tauri/src/life_stream/images/*`
- `src/features/life-stream/components/stream/*` (image display)

### Obsidian persistence format

Focus on:
- YAML/frontmatter stability (entity files)
- Stream monthly file append behavior
- idempotency (re-running shouldn’t duplicate)

Relevant code:
- Rust persistence layer under `src-tauri/src/life_stream/persistence/*`
- Obsidian schema expectations described in **[DATA_MODELS.md](DATA_MODELS.md)**

---

## Coverage gaps observed in this snapshot

This is *not* a criticism — it’s a map of where tests would buy you the most confidence.

### High value gaps
- **End-to-end Life Stream integration**: Tauri invoke → Rust decision engine → MCP bridge → tool parsing → UI card render  
  *(currently mostly covered by unit tests at each layer; little full-path coverage)*
- **MCP bridge failure modes**: spawn failures, timeouts, stderr noise, partial JSON lines
- **Image pipeline**: error cases + cache invalidation are not heavily tested
- **Type sync**: “TS types + Rust types must stay in sync” is mostly process-based, not enforced by tests

### Medium value gaps
- `cardHighlights.ts` appears to be used by stream UI but has no dedicated unit test in `src/**` (easy win).
- “Life OS mode vs normal mode” UI toggles could use more regression tests around layout/panel persistence.

> There *is* a `TEST_CATALOG.md` in the CodexMonitor-lifeos repo that lists many desired test cases by feature. Treat it as a backlog/roadmap for test expansion.

---

## CodexMonitor-lifeos: Rust tests

From `CodexMonitor-lifeos/src-tauri`:

```bash
cargo test
```

Rust tests in this snapshot are mostly:
- small parsing/unit helpers
- data extraction utilities

They’re valuable for locking down:
- snake_case vs camelCase input decoding
- JSON nesting rules (`result.result`)
- parsing edge cases from Codex app-server events

See **[GOTCHAS.md](GOTCHAS.md)** for why this matters.

---

## life-mcp: Jest suites

### Running tests

From `life-mcp/`:

```bash
npm test
npm run test:watch
npm run test:coverage

npm run test:unit
npm run test:integration
npm run test:schema
npm run test:regression
```

### What to test in life-mcp

Because this server has many tools and external integrations, tests should focus on:

| Test type | Target | Examples |
|---|---|---|
| Unit | local pure logic | keyword routing, payload normalization, parsing |
| Integration | adapter boundaries | Supabase client calls, Sheets reads/writes, Obsidian file writes |
| Schema | response format | enforce dual output shape (human text + JSON) |
| Regression | past bugs | timezone, dedupe, tool-name mismatches |

### Mocking external services

Preferred pattern:
- Keep fixtures of raw third-party responses (TMDB, Sheets, Supabase)
- Build a thin mock client layer (`src/clients/*`) that can be swapped in tests
- For tools, assert on **normalized** output objects, not raw responses

---

## Suggested future additions (optional)

If you want a “confidence rocket booster”:

- **Golden fixtures** for each domain tool output → expected card payload JSON
- **Contract tests**: Node tool schema ↔ Rust tool parser ↔ TS interface  
  (see **[DATA_MODELS.md](DATA_MODELS.md)** mapping table)
- A tiny **smoke test harness** that spawns life-mcp + runs a handful of tool calls through the Rust bridge
