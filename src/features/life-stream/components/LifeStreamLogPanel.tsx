import { useCallback, useEffect, useState } from "react";
import { readLifeStreamLog } from "../../../services/tauri";
import "./LifeStreamLogPanel.css";

type LifeStreamLogPanelProps = {
  workspaceId: string | null;
  isOpen: boolean;
  onClose: () => void;
};

export function LifeStreamLogPanel({
  workspaceId,
  isOpen,
  onClose,
}: LifeStreamLogPanelProps) {
  const [lines, setLines] = useState<string[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const loadLogs = useCallback(async () => {
    if (!workspaceId) {
      setLines([]);
      setError("No Life OS workspace available.");
      return;
    }
    setIsLoading(true);
    setError(null);
    try {
      const data = await readLifeStreamLog(workspaceId, 200);
      setLines(Array.isArray(data) ? data : []);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsLoading(false);
    }
  }, [workspaceId]);

  useEffect(() => {
    if (isOpen) {
      void loadLogs();
    }
  }, [isOpen, loadLogs]);

  if (!isOpen) {
    return null;
  }

  return (
    <section className="life-stream-log-panel" aria-live="polite">
      <div className="life-stream-log-panel__header">
        <span className="life-stream-log-panel__title">Life Stream Log</span>
        <div className="life-stream-log-panel__actions">
          <button
            type="button"
            className="life-stream-log-panel__button"
            onClick={loadLogs}
            disabled={isLoading}
          >
            Refresh
          </button>
          <button
            type="button"
            className="life-stream-log-panel__button"
            onClick={onClose}
          >
            Close
          </button>
        </div>
      </div>

      {isLoading && <div className="life-stream-log-panel__status">Loading…</div>}
      {error && <div className="life-stream-log-panel__error">{error}</div>}
      {!isLoading && !error && (
        <pre className="life-stream-log-panel__body">
          {lines.length === 0 ? "No log entries yet." : lines.join("\n")}
        </pre>
      )}
    </section>
  );
}
