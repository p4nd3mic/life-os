import { useEffect, useMemo, useState } from "react";
import type { ImageCandidate, ImageCandidateResponse } from "../../types";

type ImageAttachSelectionOptions = {
  setContextOverride: boolean;
  updateEntityFile: boolean;
  updateEntityEmbed: boolean;
};

type ImageAttachSheetProps = {
  open: boolean;
  loading: boolean;
  error: string | null;
  response: ImageCandidateResponse | null;
  contextHint?: string | null;
  onClose: () => void;
  onBrowse: () => void;
  onSelectCandidate: (
    candidate: ImageCandidate,
    options: ImageAttachSelectionOptions,
  ) => void;
};

const DEFAULT_VISIBLE_COUNT = 3;

function sourceKindLabel(sourceKind: string): string {
  switch (sourceKind) {
    case "managed_local":
      return "vault";
    case "external_local":
      return "external";
    case "provider_tmdb":
      return "tmdb";
    case "manual_browse":
      return "manual";
    default:
      return sourceKind.replace(/_/g, " ");
  }
}

export function ImageAttachSheet({
  open,
  loading,
  error,
  response,
  contextHint,
  onClose,
  onBrowse,
  onSelectCandidate,
}: ImageAttachSheetProps) {
  const [showAll, setShowAll] = useState(false);
  const [setContextOverride, setSetContextOverride] = useState(false);
  const [updateEntityFile, setUpdateEntityFile] = useState(true);
  const [updateEntityEmbed, setUpdateEntityEmbed] = useState(false);

  useEffect(() => {
    if (!open) {
      setShowAll(false);
      setSetContextOverride(false);
      setUpdateEntityFile(true);
      setUpdateEntityEmbed(false);
    }
  }, [open]);

  useEffect(() => {
    setShowAll(false);
    setSetContextOverride(Boolean(contextHint));
    setUpdateEntityFile(true);
    setUpdateEntityEmbed(false);
  }, [contextHint, response?.entityKey]);

  const candidates = response?.candidates ?? [];
  const visibleCandidates = useMemo(() => {
    if (showAll) return candidates;
    return candidates.slice(0, DEFAULT_VISIBLE_COUNT);
  }, [candidates, showAll]);

  if (!open) {
    return null;
  }

  return (
    <div className="life-image-attach-sheet" data-no-toggle>
      <div className="life-image-attach-sheet__header">
        <div className="life-image-attach-sheet__title">
          📷 Set image
          {response?.entityName ? ` · ${response.entityName}` : ""}
        </div>
        <button
          type="button"
          className="life-image-attach-sheet__close"
          onClick={onClose}
          aria-label="Close image picker"
        >
          ✕
        </button>
      </div>

      <div className="life-image-attach-sheet__toggles">
        <label className="life-image-attach-sheet__toggle-row">
          <input
            type="checkbox"
            checked={setContextOverride}
            onChange={(event) => setSetContextOverride(event.target.checked)}
            disabled={!contextHint}
          />
          <span>
            Context override {contextHint ? "for this prompt" : "(no context node selected)"}
          </span>
        </label>
        <label className="life-image-attach-sheet__toggle-row">
          <input
            type="checkbox"
            checked={updateEntityFile}
            onChange={(event) => setUpdateEntityFile(event.target.checked)}
          />
          <span>Update entity frontmatter image field</span>
        </label>
        <label className="life-image-attach-sheet__toggle-row">
          <input
            type="checkbox"
            checked={updateEntityEmbed}
            onChange={(event) => setUpdateEntityEmbed(event.target.checked)}
          />
          <span>Update entity embed block</span>
        </label>
      </div>

      {loading && <div className="life-image-attach-sheet__status">Scanning local + remote sources…</div>}
      {!loading && error && <div className="life-image-attach-sheet__status is-error">{error}</div>}
      {!loading && !error && candidates.length === 0 && (
        <div className="life-image-attach-sheet__status">No candidates found yet.</div>
      )}

      {!loading && !error && candidates.length > 0 && (
        <>
          <ul className="life-image-attach-sheet__list">
            {visibleCandidates.map((candidate) => (
              <li key={`${candidate.sourcePath}::${candidate.sourceKind}`}>
                <button
                  type="button"
                  className="life-image-attach-sheet__candidate"
                  onClick={() =>
                    onSelectCandidate(candidate, {
                      setContextOverride,
                      updateEntityFile,
                      updateEntityEmbed,
                    })
                  }
                >
                  <span className="life-image-attach-sheet__candidate-name">
                    {candidate.fileName}
                  </span>
                  <span className="life-image-attach-sheet__candidate-meta">
                    <span className="life-image-attach-sheet__candidate-source">
                      {sourceKindLabel(candidate.sourceKind)}
                    </span>
                    score {candidate.score}
                    {candidate.reason.length > 0
                      ? ` • ${candidate.reason.join(", ")}`
                      : ""}
                  </span>
                </button>
              </li>
            ))}
          </ul>
          {candidates.length > DEFAULT_VISIBLE_COUNT && (
            <button
              type="button"
              className="life-image-attach-sheet__toggle"
              onClick={() => setShowAll((prev) => !prev)}
            >
              {showAll
                ? `Show top ${DEFAULT_VISIBLE_COUNT}`
                : `Show all (${candidates.length})`}
            </button>
          )}
        </>
      )}

      <div className="life-image-attach-sheet__actions">
        <button
          type="button"
          className="life-image-attach-sheet__action is-primary"
          onClick={onBrowse}
        >
          Browse file…
        </button>
      </div>
    </div>
  );
}

export type { ImageAttachSelectionOptions };
