import { useMemo } from "react";
import Copy from "lucide-react/dist/esm/icons/copy";
import X from "lucide-react/dist/esm/icons/x";
import type { ExpandedSection, StreamCard } from "../types";
import "./LifeStreamTracePanel.css";

type LifeStreamTracePanelProps = {
  open: boolean;
  card: StreamCard | null;
  onClose: () => void;
};

function findSection(
  sections: ExpandedSection[] | undefined,
  predicate: (section: ExpandedSection) => boolean,
) {
  return sections?.find(predicate);
}

function findLastSection(
  sections: ExpandedSection[] | undefined,
  predicate: (section: ExpandedSection) => boolean,
) {
  if (!sections || sections.length === 0) return undefined;
  for (let i = sections.length - 1; i >= 0; i -= 1) {
    if (predicate(sections[i])) {
      return sections[i];
    }
  }
  return undefined;
}

function parseJson(raw?: string) {
  if (!raw) return null;
  try {
    return JSON.parse(raw);
  } catch {
    return null;
  }
}

export function LifeStreamTracePanel({
  open,
  card,
  onClose,
}: LifeStreamTracePanelProps) {
  const decisionSection = useMemo(
    () =>
      findSection(card?.expanded?.sections, (section) =>
        section.title.toLowerCase().includes("codex decision json"),
      ),
    [card],
  );

  const lastMcpSection = useMemo(
    () =>
      findLastSection(card?.expanded?.sections, (section) =>
        section.title.toLowerCase().startsWith("life-mcp"),
      ),
    [card],
  );

  const decisionJson = useMemo(
    () => parseJson(decisionSection?.body),
    [decisionSection?.body],
  );

  const imageQuery = useMemo(() => {
    if (!decisionJson) return null;
    return (
      decisionJson.image_query ??
      decisionJson.imageQuery ??
      decisionJson.image_query_text ??
      null
    );
  }, [decisionJson]);

  const decisionText = useMemo(() => {
    if (!decisionSection?.body) return null;
    if (decisionJson) {
      return JSON.stringify(decisionJson, null, 2);
    }
    return decisionSection.body;
  }, [decisionJson, decisionSection?.body]);

  const steps = card?.processingSteps ?? [];
  const originalInput = card?.originalInput ?? card?.expanded?.originalInput ?? "";

  const copyToClipboard = (value: string | null | undefined) => {
    if (!value) return;
    if (navigator?.clipboard?.writeText) {
      void navigator.clipboard.writeText(value);
    }
  };

  if (!open) return null;

  return (
    <div className="life-trace-overlay" role="dialog" aria-modal="true">
      <button
        type="button"
        className="life-trace-backdrop"
        aria-label="Close trace panel"
        onClick={onClose}
      />
      <div className="life-trace-sheet">
        <header className="life-trace-header">
          <div>
            <div className="life-trace-title">Reasoning Trace</div>
            <div className="life-trace-subtitle">
              {card ? card.title : "No recent card"}
            </div>
          </div>
          <button
            type="button"
            className="life-trace-close"
            onClick={onClose}
            aria-label="Close"
          >
            <X aria-hidden />
          </button>
        </header>

        <div className="life-trace-content">
          {!card && (
            <div className="life-trace-empty">No trace data yet.</div>
          )}

          {card && (
            <>
              {originalInput && (
                <section className="life-trace-section">
                  <div className="life-trace-section-header">
                    <h4>Original Input</h4>
                    <button
                      type="button"
                      className="life-trace-copy"
                      onClick={() => copyToClipboard(originalInput)}
                    >
                      <Copy aria-hidden />
                      Copy
                    </button>
                  </div>
                  <pre className="life-trace-pre">{originalInput}</pre>
                </section>
              )}

              {decisionText && (
                <section className="life-trace-section">
                  <div className="life-trace-section-header">
                    <h4>Codex Decision JSON</h4>
                    <button
                      type="button"
                      className="life-trace-copy"
                      onClick={() => copyToClipboard(decisionText)}
                    >
                      <Copy aria-hidden />
                      Copy
                    </button>
                  </div>
                  <pre className="life-trace-pre">{decisionText}</pre>
                  {imageQuery && (
                    <div className="life-trace-inline">
                      <span>Image query:</span>
                      <code>{imageQuery}</code>
                      <button
                        type="button"
                        className="life-trace-copy"
                        onClick={() => copyToClipboard(String(imageQuery))}
                      >
                        <Copy aria-hidden />
                        Copy
                      </button>
                    </div>
                  )}
                </section>
              )}

              {lastMcpSection && (
                <section className="life-trace-section">
                  <div className="life-trace-section-header">
                    <h4>Last MCP Call</h4>
                    <button
                      type="button"
                      className="life-trace-copy"
                      onClick={() => copyToClipboard(lastMcpSection.body)}
                    >
                      <Copy aria-hidden />
                      Copy
                    </button>
                  </div>
                  <div className="life-trace-muted">{lastMcpSection.title}</div>
                  <pre className="life-trace-pre">{lastMcpSection.body}</pre>
                </section>
              )}

              {steps.length > 0 && (
                <section className="life-trace-section">
                  <div className="life-trace-section-header">
                    <h4>Step-by-step Trace</h4>
                    <button
                      type="button"
                      className="life-trace-copy"
                      onClick={() => copyToClipboard(steps.join("\n"))}
                    >
                      <Copy aria-hidden />
                      Copy
                    </button>
                  </div>
                  <ul className="life-trace-steps">
                    {steps.map((step, index) => (
                      <li key={`${step}-${index}`}>{step}</li>
                    ))}
                  </ul>
                </section>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );
}
