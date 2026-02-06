import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { AccessMode, ComposerEditorSettings, WorkspaceInfo } from "../types";
import {
  listWorkspaces,
  connectWorkspace,
  lifeStreamImageBackfill,
} from "../services/tauri";
import { LifeStreamProvider } from "../features/life-stream/context/LifeStreamContext";
import { LifeTopbar } from "../features/life-stream/components/navigation/LifeTopbar";
import { LifeStreamMessageView } from "../features/life-stream/components/LifeStreamMessageView";
import { LifeStreamComposer } from "../features/life-stream/components/composer/LifeStreamComposer";
import { useModels } from "../features/models/hooks/useModels";
import { useCollaborationModes } from "../features/collaboration/hooks/useCollaborationModes";
import { useSkills } from "../features/skills/hooks/useSkills";
import { useCustomPrompts } from "../features/prompts/hooks/useCustomPrompts";
import { useWorkspaceFiles } from "../features/workspaces/hooks/useWorkspaceFiles";
import { useComposerEditorState } from "../features/composer/hooks/useComposerEditorState";
import { LifeWorkspaceErrorBoundary } from "../features/life/components/LifeWorkspaceErrorBoundary";
import { setBridgeWorkspaceId } from "./bridge";
import "./life-stream-webview.css";

type ConnectionStatus = "idle" | "connecting" | "connected" | "error";
type ModelStatus = "idle" | "loading" | "error";
type BackfillState = "idle" | "running" | "done" | "error";

const DEFAULT_EDITOR_SETTINGS: ComposerEditorSettings = {
  preset: "default",
  expandFenceOnSpace: false,
  expandFenceOnEnter: false,
  fenceLanguageTags: false,
  fenceWrapSelection: false,
  autoWrapPasteMultiline: false,
  autoWrapPasteCodeLike: false,
  continueListOnShiftEnter: false,
};

function findLifeWorkspace(workspaces: WorkspaceInfo[]): WorkspaceInfo | null {
  return (
    workspaces.find((workspace) => workspace.settings?.purpose === "life") ??
    workspaces[0] ??
    null
  );
}

export function LifeStreamWebApp() {
  const [workspace, setWorkspace] = useState<WorkspaceInfo | null>(null);
  const [connectionStatus, setConnectionStatus] = useState<ConnectionStatus>("idle");
  const [connectionError, setConnectionError] = useState<string | null>(null);
  const [accessMode, setAccessMode] = useState<AccessMode>("full-access");
  const [modelStatus, setModelStatus] = useState<ModelStatus>("idle");
  const [modelError, setModelError] = useState<string | null>(null);
  const [backfillState, setBackfillState] = useState<BackfillState>("idle");
  const [backfillMessage, setBackfillMessage] = useState<string | null>(null);
  const textareaRef = useRef<HTMLTextAreaElement | null>(null);

  const { isExpanded, toggleExpanded } = useComposerEditorState();

  const loadWorkspaces = useCallback(async () => {
    setConnectionStatus("connecting");
    setConnectionError(null);
    try {
      const entries = await listWorkspaces();
      const lifeWorkspace = findLifeWorkspace(entries);
      if (!lifeWorkspace) {
        setConnectionStatus("error");
        setConnectionError("No Life OS workspace configured.");
        return;
      }
      setWorkspace(lifeWorkspace);
      setBridgeWorkspaceId(lifeWorkspace.id);

      try {
        await connectWorkspace(lifeWorkspace.id);
        setWorkspace({ ...lifeWorkspace, connected: true });
        setConnectionStatus("connected");
      } catch (error) {
        setConnectionStatus("error");
        setConnectionError(error instanceof Error ? error.message : String(error));
      }
    } catch (error) {
      setConnectionStatus("error");
      setConnectionError(error instanceof Error ? error.message : String(error));
    }
  }, []);

  useEffect(() => {
    void loadWorkspaces();
  }, [loadWorkspaces]);

  useEffect(() => {
    setBridgeWorkspaceId(workspace?.id ?? null);
  }, [workspace?.id]);

  const {
    models,
    selectedModelId,
    setSelectedModelId,
    reasoningOptions,
    selectedEffort,
    setSelectedEffort,
    refreshModels,
  } = useModels({ activeWorkspace: workspace });

  const {
    collaborationModes,
    selectedCollaborationModeId,
    setSelectedCollaborationModeId,
    refreshCollaborationModes,
  } = useCollaborationModes({
    activeWorkspace: workspace,
    enabled: true,
  });

  const { skills } = useSkills({ activeWorkspace: workspace });
  const { prompts } = useCustomPrompts({ activeWorkspace: workspace });
  const { files } = useWorkspaceFiles({ activeWorkspace: workspace });

  const handleRefreshModels = useCallback(async () => {
    setModelStatus("loading");
    setModelError(null);
    try {
      await refreshModels();
      setModelStatus("idle");
    } catch (error) {
      setModelStatus("error");
      setModelError(error instanceof Error ? error.message : String(error));
    }
  }, [refreshModels]);

  useEffect(() => {
    if (connectionStatus !== "connected") {
      return;
    }
    void handleRefreshModels();
    void refreshCollaborationModes();
  }, [connectionStatus, handleRefreshModels, refreshCollaborationModes]);

  const editorSettings = useMemo(() => DEFAULT_EDITOR_SETTINGS, []);

  const runImageBackfill = useCallback(async () => {
    if (!workspace?.id || backfillState === "running") {
      return;
    }

    setBackfillState("running");
    setBackfillMessage("🖼️ Backfilling entity image metadata...");
    try {
      const summary = await lifeStreamImageBackfill(workspace.id, false);
      const message = `✅ Backfill complete · updated ${summary.updated}, skipped ${summary.skipped}, failed ${summary.failed}`;
      setBackfillState("done");
      setBackfillMessage(message);
      if (summary.failed > 0 && summary.errors.length > 0) {
        console.error("life_stream_image_backfill errors:", summary.errors);
      }
    } catch (error) {
      setBackfillState("error");
      setBackfillMessage(
        `⚠️ Backfill failed: ${error instanceof Error ? error.message : String(error)}`,
      );
    }
  }, [backfillState, workspace?.id]);

  useEffect(() => {
    if (!backfillMessage) {
      return;
    }
    const timeout = window.setTimeout(() => {
      setBackfillMessage(null);
      if (backfillState !== "running") {
        setBackfillState("idle");
      }
    }, 4800);
    return () => window.clearTimeout(timeout);
  }, [backfillMessage, backfillState]);

  return (
    <div className="app life-mode life-webview">
      <LifeWorkspaceErrorBoundary>
        <LifeStreamProvider workspaceId={workspace?.id ?? null}>
          <div className="life-stream-shell life-stream-shell--task-dock">
            <LifeTopbar
              actionsNode={(
                <div className="life-topbar-image-actions">
                  <button
                    type="button"
                    className="life-topbar-image-actions__button"
                    disabled={!workspace?.id || backfillState === "running"}
                    onClick={() => {
                      void runImageBackfill();
                    }}
                  >
                    {backfillState === "running" ? "⏳ Backfilling..." : "🖼️ Backfill images"}
                  </button>
                </div>
              )}
            />
            {backfillMessage && (
              <div
                className={`life-stream-backfill-toast${
                  backfillState === "error" ? " is-error" : ""
                }`}
                role="status"
              >
                {backfillMessage}
              </div>
            )}
            <LifeStreamMessageView />
            <div className="life-stream-composer-area">
              <LifeStreamComposer
                workspaceId={workspace?.id ?? null}
                models={models}
                selectedModelId={selectedModelId}
                onSelectModel={(id) => setSelectedModelId(id)}
                reasoningOptions={reasoningOptions}
                selectedEffort={selectedEffort}
                onSelectEffort={(effort) => setSelectedEffort(effort)}
                accessMode={accessMode}
                onSelectAccessMode={setAccessMode}
                collaborationModes={collaborationModes}
                selectedCollaborationModeId={selectedCollaborationModeId}
                onSelectCollaborationMode={setSelectedCollaborationModeId}
                connectionStatus={connectionStatus}
                connectionError={connectionError}
                onRetryConnect={loadWorkspaces}
                modelStatus={modelStatus}
                modelError={modelError}
                onRefreshModels={handleRefreshModels}
                skills={skills}
                prompts={prompts}
                files={files}
                editorSettings={editorSettings}
                editorExpanded={isExpanded}
                onToggleEditorExpanded={toggleExpanded}
                textareaRef={textareaRef}
                steerEnabled={false}
                dictationEnabled={false}
                dictationState="idle"
                dictationLevel={0}
                onToggleDictation={() => {}}
                onOpenDictationSettings={() => {}}
                dictationTranscript={null}
                onDictationTranscriptHandled={() => {}}
                dictationError={null}
                onDismissDictationError={() => {}}
                dictationHint={null}
                onDismissDictationHint={() => {}}
                attachmentsEnabled={false}
              />
            </div>
          </div>
        </LifeStreamProvider>
      </LifeWorkspaceErrorBoundary>
    </div>
  );
}
