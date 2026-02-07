import { useCallback, useMemo, useState, type RefObject } from "react";
import type {
  AccessMode,
  ComposerEditorSettings,
  CollaborationModeOption,
  CustomPromptOption,
  DictationTranscript,
  SkillOption,
} from "../../../../types";
import { Composer } from "../../../composer/components/Composer";
import { LifeStreamTracePanel } from "../LifeStreamTracePanel";
import { LifeStreamLogPanel } from "../LifeStreamLogPanel";
import { useLifeStreamContext } from "../../context/LifeStreamContext";
import { useComposerImages } from "../../../composer/hooks/useComposerImages";
import "./LifeStreamComposer.css";

type LifeConnectStatus = "idle" | "connecting" | "connected" | "error";
type ModelLoadStatus = "idle" | "loading" | "error";

type LifeStreamComposerProps = {
  workspaceId: string | null;
  models: { id: string; displayName: string; model: string }[];
  selectedModelId: string | null;
  onSelectModel: (id: string) => void;
  reasoningOptions: string[];
  selectedEffort: string | null;
  onSelectEffort: (effort: string) => void;
  accessMode: AccessMode;
  onSelectAccessMode: (mode: AccessMode) => void;
  collaborationModes: CollaborationModeOption[];
  selectedCollaborationModeId: string | null;
  onSelectCollaborationMode: (id: string | null) => void;
  connectionStatus: LifeConnectStatus;
  connectionError: string | null;
  onRetryConnect: () => void;
  modelStatus: ModelLoadStatus;
  modelError: string | null;
  onRefreshModels: () => void;
  skills: SkillOption[];
  prompts: CustomPromptOption[];
  files: string[];
  editorSettings: ComposerEditorSettings;
  editorExpanded: boolean;
  onToggleEditorExpanded: () => void;
  textareaRef: RefObject<HTMLTextAreaElement | null>;
  steerEnabled: boolean;
  dictationEnabled: boolean;
  dictationState: "idle" | "listening" | "processing";
  dictationLevel: number;
  onToggleDictation: () => void;
  onOpenDictationSettings: () => void;
  dictationTranscript: DictationTranscript | null;
  onDictationTranscriptHandled: (id: string) => void;
  dictationError: string | null;
  onDismissDictationError: () => void;
  dictationHint: string | null;
  onDismissDictationHint: () => void;
  attachmentsEnabled?: boolean;
};

export function LifeStreamComposer({
  workspaceId,
  models,
  selectedModelId,
  onSelectModel,
  reasoningOptions,
  selectedEffort,
  onSelectEffort,
  accessMode,
  onSelectAccessMode,
  collaborationModes,
  selectedCollaborationModeId,
  onSelectCollaborationMode,
  connectionStatus,
  connectionError,
  onRetryConnect,
  modelStatus,
  modelError,
  onRefreshModels,
  skills,
  prompts,
  files,
  editorSettings,
  editorExpanded,
  onToggleEditorExpanded,
  textareaRef,
  steerEnabled,
  dictationEnabled,
  dictationState,
  dictationLevel,
  onToggleDictation,
  onOpenDictationSettings,
  dictationTranscript,
  onDictationTranscriptHandled,
  dictationError,
  onDismissDictationError,
  dictationHint,
  onDismissDictationHint,
  attachmentsEnabled = true,
}: LifeStreamComposerProps) {
  const {
    submit,
    submitStatus,
    cards,
    taskDock,
    setTaskDockCollapsed,
    setTaskDockHidden,
  } = useLifeStreamContext();
  const [draftText, setDraftText] = useState("");
  const [traceOpen, setTraceOpen] = useState(false);
  const [showLogs, setShowLogs] = useState(false);
  const isDisabled = workspaceId === null;
  const { activeImages, attachImages, pickImages, removeImage, clearActiveImages } =
    useComposerImages({
      activeThreadId: null,
      activeWorkspaceId: workspaceId,
    });

  const handlePickImages = useCallback(async () => {
    if (!attachmentsEnabled) {
      if (typeof window !== "undefined") {
        window.alert("Attachments are not supported yet.");
      }
      return;
    }
    await pickImages();
  }, [attachmentsEnabled, pickImages]);

  const historyKey = useMemo(() => {
    return workspaceId ? `life-os-${workspaceId}` : "life-os";
  }, [workspaceId]);

  const selectedCollaborationMode = useMemo(
    () =>
      collaborationModes.find((mode) => mode.id === selectedCollaborationModeId) ?? null,
    [collaborationModes, selectedCollaborationModeId],
  );

  const resolvedModel = useMemo(() => {
    if (!selectedModelId) return null;
    const matched = models.find((model) => model.id === selectedModelId);
    const candidate = (matched?.model ?? selectedModelId).trim();
    return candidate.length > 0 ? candidate : null;
  }, [models, selectedModelId]);

  const resolvedEffort = useMemo(() => {
    if (!selectedEffort) return null;
    const candidate = selectedEffort.trim();
    return candidate.length > 0 ? candidate : null;
  }, [selectedEffort]);

  const buildPayload = useCallback((text: string, images: string[]) => {
    const trimmed = text.trim();
    if (images.length === 0) {
      return trimmed;
    }
    const attachmentBlock = images.map((path) => `- ${path}`).join("\n");
    if (!trimmed) {
      return `Uploaded images\n\n[images]\n${attachmentBlock}`;
    }
    return `${trimmed}\n\n[images]\n${attachmentBlock}`;
  }, []);

  const handleSend = useCallback(
    (text: string, images: string[]) => {
      const payload = buildPayload(text, images);
      if (!payload) {
        return;
      }
      void submit(payload, {
        modelId: resolvedModel,
        effort: resolvedEffort,
        accessMode,
        collaborationMode: selectedCollaborationMode?.value ?? null,
      });
      if (attachmentsEnabled) {
        clearActiveImages();
      }
    },
    [
      accessMode,
      attachmentsEnabled,
      buildPayload,
      clearActiveImages,
      resolvedEffort,
      resolvedModel,
      selectedCollaborationMode,
      submit,
    ],
  );

  const handleQueue = useCallback(
    (text: string, images: string[]) => {
      handleSend(text, images);
    },
    [handleSend],
  );

  const showConnectionStatus =
    connectionStatus === "connecting" || connectionStatus === "error";
  const showModelStatus =
    connectionStatus === "connected" &&
    (models.length === 0 || modelStatus !== "idle");

  const showSubmitStatus = submitStatus.state !== "idle";

  const modelStatusMessage = useMemo(() => {
    if (modelStatus === "loading") {
      return "Loading models from Codex...";
    }
    if (modelStatus === "error") {
      return modelError
        ? `Model load failed: ${modelError}`
        : "Model load failed. Try refresh.";
    }
    if (models.length === 0) {
      return "No models loaded yet.";
    }
    return null;
  }, [modelError, modelStatus, models.length]);

  const traceCard = useMemo(() => {
    if (cards.length === 0) return null;
    const richCard = cards.find(
      (card) =>
        (card.expanded?.sections?.length ?? 0) > 0 ||
        (card.processingSteps?.length ?? 0) > 0,
    );
    return richCard ?? cards[0] ?? null;
  }, [cards]);

  return (
    <div className="life-stream-composer-shell">
      <LifeStreamTracePanel
        open={traceOpen}
        onClose={() => setTraceOpen(false)}
        card={traceCard}
      />
      <LifeStreamLogPanel
        workspaceId={workspaceId}
        isOpen={showLogs}
        onClose={() => setShowLogs(false)}
      />
      {showSubmitStatus && (
        <div
          className={`life-stream-composer-status ${
            submitStatus.state === "error" ? "is-error" : "is-loading"
          }`}
        >
          <span>{submitStatus.message ?? "Message received"}</span>
        </div>
      )}
      {showConnectionStatus && (
        <div
          className={`life-stream-composer-status ${
            connectionStatus === "error" ? "is-error" : "is-loading"
          }`}
        >
          <span>
            {connectionStatus === "connecting"
              ? "Connecting to Codex..."
              : connectionError ?? "Failed to connect to Codex."}
          </span>
          {connectionStatus === "error" && (
            <button
              type="button"
              className="life-stream-composer-status__action"
              onClick={onRetryConnect}
            >
              Retry
            </button>
          )}
        </div>
      )}

      {showModelStatus && modelStatusMessage && (
        <div
          className={`life-stream-composer-status ${
            modelStatus === "error" ? "is-error" : "is-loading"
          }`}
        >
          <span>{modelStatusMessage}</span>
          {(modelStatus === "error" || models.length === 0) && (
            <button
              type="button"
              className="life-stream-composer-status__action"
              onClick={onRefreshModels}
            >
              Refresh
            </button>
          )}
        </div>
      )}

      <div className="life-stream-composer-actions">
        <button
          type="button"
          className="life-stream-log-toggle"
          onClick={() => setTraceOpen((prev) => !prev)}
        >
          {traceOpen ? "Hide Trace" : "Show Trace"}
        </button>
        <button
          type="button"
          className="life-stream-log-toggle"
          onClick={() => setShowLogs((prev) => !prev)}
        >
          {showLogs ? "Hide Logs" : "Show Logs"}
        </button>
        <button
          type="button"
          className={`life-stream-log-toggle${taskDock.hidden ? "" : " is-active"}`}
          onClick={() => {
            if (taskDock.hidden) {
              setTaskDockHidden(false);
              setTaskDockCollapsed(false);
              return;
            }
            setTaskDockHidden(true);
          }}
        >
          {taskDock.hidden ? "Show Tasks" : "Hide Tasks"}
        </button>
      </div>

      <Composer
        onSend={handleSend}
        onQueue={handleQueue}
        onStop={() => {}}
        canStop={false}
        disabled={isDisabled}
        isProcessing={false}
        steerEnabled={steerEnabled}
        collaborationModes={collaborationModes}
        selectedCollaborationModeId={selectedCollaborationModeId}
        onSelectCollaborationMode={onSelectCollaborationMode}
        models={models}
        selectedModelId={selectedModelId}
        onSelectModel={onSelectModel}
        reasoningOptions={reasoningOptions}
        selectedEffort={selectedEffort}
        onSelectEffort={onSelectEffort}
        accessMode={accessMode}
        onSelectAccessMode={onSelectAccessMode}
        skills={skills}
        prompts={prompts}
        files={files}
        contextUsage={null}
        queuedMessages={[]}
        sendLabel="Send"
        draftText={draftText}
        onDraftChange={setDraftText}
        historyKey={historyKey}
        attachedImages={attachmentsEnabled ? activeImages : []}
        onPickImages={handlePickImages}
        onAttachImages={attachmentsEnabled ? attachImages : undefined}
        onRemoveImage={attachmentsEnabled ? removeImage : undefined}
        textareaRef={textareaRef}
        editorSettings={editorSettings}
        editorExpanded={editorExpanded}
        onToggleEditorExpanded={onToggleEditorExpanded}
        dictationEnabled={dictationEnabled}
        dictationState={dictationState}
        dictationLevel={dictationLevel}
        onToggleDictation={onToggleDictation}
        onOpenDictationSettings={onOpenDictationSettings}
        dictationTranscript={dictationTranscript}
        onDictationTranscriptHandled={onDictationTranscriptHandled}
        dictationError={dictationError}
        onDismissDictationError={onDismissDictationError}
        dictationHint={dictationHint}
        onDismissDictationHint={onDismissDictationHint}
      />
    </div>
  );
}
