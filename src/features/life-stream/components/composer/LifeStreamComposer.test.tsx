// @vitest-environment jsdom
import { createRef } from "react";
import { render, screen, fireEvent } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { ComposerEditorSettings } from "../../../../types";
import { LifeStreamComposer } from "./LifeStreamComposer";
import type { StreamCard } from "../../types";

const submitMock = vi.fn();
const mockContext = {
  submit: submitMock,
  submitStatus: { state: "idle" as const },
  clearSubmitStatus: vi.fn(),
  cards: [] as StreamCard[],
};

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    onDragDropEvent: () => Promise.resolve(() => {}),
  }),
}));

vi.mock("../../context/LifeStreamContext", () => ({
  useLifeStreamContext: () => mockContext,
}));

const editorSettings: ComposerEditorSettings = {
  preset: "default",
  expandFenceOnSpace: false,
  expandFenceOnEnter: false,
  fenceLanguageTags: false,
  fenceWrapSelection: false,
  autoWrapPasteMultiline: false,
  autoWrapPasteCodeLike: false,
  continueListOnShiftEnter: false,
};

describe("LifeStreamComposer", () => {
  it("submits text to the life stream", () => {
    mockContext.cards = [];
    render(
      <LifeStreamComposer
        workspaceId="life-os"
        models={[{ id: "gpt", displayName: "GPT", model: "gpt" }]}
        selectedModelId="gpt"
        onSelectModel={vi.fn()}
        reasoningOptions={["medium"]}
        selectedEffort="medium"
        onSelectEffort={vi.fn()}
        accessMode="full-access"
        onSelectAccessMode={vi.fn()}
        collaborationModes={[]}
        selectedCollaborationModeId={null}
        onSelectCollaborationMode={vi.fn()}
        connectionStatus="connected"
        connectionError={null}
        onRetryConnect={vi.fn()}
        modelStatus="idle"
        modelError={null}
        onRefreshModels={vi.fn()}
        skills={[]}
        prompts={[]}
        files={[]}
        editorSettings={editorSettings}
        editorExpanded={false}
        onToggleEditorExpanded={vi.fn()}
        textareaRef={createRef<HTMLTextAreaElement>()}
        steerEnabled={false}
        dictationEnabled={false}
        dictationState="idle"
        dictationLevel={0}
        onToggleDictation={vi.fn()}
        onOpenDictationSettings={vi.fn()}
        dictationTranscript={null}
        onDictationTranscriptHandled={vi.fn()}
        dictationError={null}
        onDismissDictationError={vi.fn()}
        dictationHint={null}
        onDismissDictationHint={vi.fn()}
      />,
    );

    const textarea = screen.getByPlaceholderText("Ask Codex to do something...");
    fireEvent.change(textarea, { target: { value: "Watched Cowboy Bebop ep 5" } });

    fireEvent.click(screen.getByRole("button", { name: "Send" }));

    expect(submitMock).toHaveBeenCalledWith("Watched Cowboy Bebop ep 5", {
      modelId: "gpt",
      effort: "medium",
      accessMode: "full-access",
      collaborationMode: null,
    });
  });

  it("shows connecting status", () => {
    mockContext.cards = [];
    render(
      <LifeStreamComposer
        workspaceId="life-os"
        models={[]}
        selectedModelId={null}
        onSelectModel={vi.fn()}
        reasoningOptions={[]}
        selectedEffort={null}
        onSelectEffort={vi.fn()}
        accessMode="full-access"
        onSelectAccessMode={vi.fn()}
        collaborationModes={[]}
        selectedCollaborationModeId={null}
        onSelectCollaborationMode={vi.fn()}
        connectionStatus="connecting"
        connectionError={null}
        onRetryConnect={vi.fn()}
        modelStatus="idle"
        modelError={null}
        onRefreshModels={vi.fn()}
        skills={[]}
        prompts={[]}
        files={[]}
        editorSettings={editorSettings}
        editorExpanded={false}
        onToggleEditorExpanded={vi.fn()}
        textareaRef={createRef<HTMLTextAreaElement>()}
        steerEnabled={false}
        dictationEnabled={false}
        dictationState="idle"
        dictationLevel={0}
        onToggleDictation={vi.fn()}
        onOpenDictationSettings={vi.fn()}
        dictationTranscript={null}
        onDictationTranscriptHandled={vi.fn()}
        dictationError={null}
        onDismissDictationError={vi.fn()}
        dictationHint={null}
        onDismissDictationHint={vi.fn()}
      />,
    );

    expect(screen.getByText("Connecting to Codex...")).toBeTruthy();
  });

  it("opens the reasoning trace panel", () => {
    const now = new Date().toISOString();
    mockContext.cards = [
      {
        id: "card-1",
        occurredAt: now,
        createdAt: now,
        updatedAt: now,
        version: 1,
        cardType: "generic",
        domain: "general",
        emoji: "💬",
        state: "complete",
        title: "Response",
        summary: "Codex responding",
        expanded: {
          sections: [
            {
              title: "Codex Decision JSON",
              body: "{\"reply\":\"ok\"}",
            },
          ],
          actions: [],
        },
      },
    ];

    render(
      <LifeStreamComposer
        workspaceId="life-os"
        models={[{ id: "gpt", displayName: "GPT", model: "gpt" }]}
        selectedModelId="gpt"
        onSelectModel={vi.fn()}
        reasoningOptions={["medium"]}
        selectedEffort="medium"
        onSelectEffort={vi.fn()}
        accessMode="full-access"
        onSelectAccessMode={vi.fn()}
        collaborationModes={[]}
        selectedCollaborationModeId={null}
        onSelectCollaborationMode={vi.fn()}
        connectionStatus="connected"
        connectionError={null}
        onRetryConnect={vi.fn()}
        modelStatus="idle"
        modelError={null}
        onRefreshModels={vi.fn()}
        skills={[]}
        prompts={[]}
        files={[]}
        editorSettings={editorSettings}
        editorExpanded={false}
        onToggleEditorExpanded={vi.fn()}
        textareaRef={createRef<HTMLTextAreaElement>()}
        steerEnabled={false}
        dictationEnabled={false}
        dictationState="idle"
        dictationLevel={0}
        onToggleDictation={vi.fn()}
        onOpenDictationSettings={vi.fn()}
        dictationTranscript={null}
        onDictationTranscriptHandled={vi.fn()}
        dictationError={null}
        onDismissDictationError={vi.fn()}
        dictationHint={null}
        onDismissDictationHint={vi.fn()}
      />,
    );

    const traceButtons = screen.getAllByRole("button", { name: "Reasoning trace" });
    fireEvent.click(traceButtons[0]);
    expect(screen.getByText("Reasoning Trace")).toBeTruthy();
  });
});
