import { useLifeStreamContextOptional } from "../../context/LifeStreamContext";
import { DayPicker } from "./DayPicker";
import { EmojiFilters } from "./EmojiFilters";
import "./LifeStreamHeaderControls.css";

export function LifeStreamHeaderControls() {
  const context = useLifeStreamContextOptional();

  if (!context) {
    return null;
  }

  const {
    currentDate,
    goToPreviousDay,
    goToNextDay,
    goToToday,
    semanticRegenerationStatus,
    regenerateSemanticsForCurrentDate,
    dayThreadDebugStatus,
    inspectDayThreadForCurrentDate,
    resetDayThreadAndRebuildForCurrentDate,
    authHealthStatus,
    checkAuthHealth,
    imageAutoFetchStatus,
    codexUsageGuard,
    resetCodexUsageGuardBaseline,
    autoFetchImagesForCurrentDate,
    activeFilters,
    toggleFilter,
    clearFilters,
  } = context;

  const formatPercent = (value?: number) =>
    typeof value === "number" && Number.isFinite(value) ? `${value.toFixed(1)}%` : "—";

  const formatResetLabel = (value?: number | null) => {
    if (typeof value !== "number" || !Number.isFinite(value)) {
      return "reset —";
    }
    const msUntilReset = Math.max(0, value - Date.now());
    const minutes = Math.floor(msUntilReset / 60_000);
    const hours = Math.floor(minutes / 60);
    if (hours > 0) {
      return `reset in ${hours}h ${minutes % 60}m`;
    }
    return `reset in ${minutes}m`;
  };

  const codexUsageTrendClass =
    codexUsageGuard.trend === "surging"
      ? "is-surging"
      : codexUsageGuard.trend === "active"
        ? "is-active"
        : "is-idle";

  return (
    <div className="life-stream-header-controls" data-tauri-drag-region="false">
      <DayPicker
        currentDate={currentDate}
        onPrevious={goToPreviousDay}
        onNext={goToNextDay}
        onToday={goToToday}
      />
      <div className="life-card life-stream-semantic-actions">
        <div className="life-stream-semantic-actions__buttons">
          <button
            type="button"
            className="life-segment-button life-stream-semantic-actions__button"
            onClick={() => {
              void regenerateSemanticsForCurrentDate();
            }}
            disabled={semanticRegenerationStatus.state === "running"}
          >
            {semanticRegenerationStatus.state === "running"
              ? "⏳ Rebuilding..."
              : "🧠 Rebuild semantics"}
          </button>
          <button
            type="button"
            className="life-segment-button life-stream-semantic-actions__button"
            onClick={() => {
              void inspectDayThreadForCurrentDate();
            }}
            disabled={dayThreadDebugStatus.state === "running"}
          >
            {dayThreadDebugStatus.state === "running"
              ? "🧪 Inspecting..."
              : "🧪 Day-thread debug"}
          </button>
          <button
            type="button"
            className="life-segment-button life-stream-semantic-actions__button"
            onClick={() => {
              void resetDayThreadAndRebuildForCurrentDate();
            }}
            disabled={dayThreadDebugStatus.state === "running"}
          >
            {dayThreadDebugStatus.state === "running"
              ? "♻️ Resetting..."
              : "♻️ Reset day-thread"}
          </button>
          <button
            type="button"
            className={`life-segment-button life-stream-semantic-actions__button life-stream-semantic-actions__button--auth-${authHealthStatus.state}`}
            onClick={() => {
              void checkAuthHealth();
            }}
            disabled={authHealthStatus.state === "checking"}
          >
            {authHealthStatus.state === "checking"
              ? "🩺 Checking auth…"
              : authHealthStatus.state === "healthy"
                ? "🩺 Auth: healthy"
                : authHealthStatus.state === "unauthorized"
                  ? "🩺 Auth: login needed"
                  : "🩺 Auth check"}
          </button>
          <button
            type="button"
            className="life-segment-button life-stream-semantic-actions__button"
            onClick={() => {
              void autoFetchImagesForCurrentDate("review_first");
            }}
            disabled={imageAutoFetchStatus.state === "running"}
          >
            {imageAutoFetchStatus.state === "running"
              ? "🖼️ Fetching..."
              : "🖼️ Fetch images (ask)"}
          </button>
          <button
            type="button"
            className="life-segment-button life-stream-semantic-actions__button"
            onClick={() => {
              void autoFetchImagesForCurrentDate("auto_apply");
            }}
            disabled={imageAutoFetchStatus.state === "running"}
          >
            ⚡ Auto-apply images
          </button>
        </div>
        {semanticRegenerationStatus.message && (
          <span
            className={`life-stream-semantic-actions__status${
              semanticRegenerationStatus.state === "error" ? " is-error" : ""
            }`}
            role="status"
          >
            {semanticRegenerationStatus.message}
          </span>
        )}
        {dayThreadDebugStatus.message && (
          <span
            className={`life-stream-semantic-actions__status life-stream-semantic-actions__status--debug${
              dayThreadDebugStatus.state === "error" ? " is-error" : ""
            }`}
            role="status"
          >
            {dayThreadDebugStatus.message}
          </span>
        )}
        {authHealthStatus.message && (
          <span
            className={`life-stream-semantic-actions__status life-stream-semantic-actions__status--auth life-stream-semantic-actions__status--auth-${authHealthStatus.state}${
              authHealthStatus.state === "error" ||
              authHealthStatus.state === "unauthorized"
                ? " is-error"
                : ""
            }`}
            role="status"
          >
            {authHealthStatus.message}
          </span>
        )}
        {imageAutoFetchStatus.message && (
          <span
            className={`life-stream-semantic-actions__status life-stream-semantic-actions__status--image${
              imageAutoFetchStatus.state === "error" ? " is-error" : ""
            }`}
            role="status"
          >
            {imageAutoFetchStatus.message}
          </span>
        )}
        <div
          className={`life-stream-semantic-actions__usage-panel ${codexUsageTrendClass}`}
          role="status"
          aria-live="polite"
        >
          <div className="life-stream-semantic-actions__usage-panel-header">
            <span>🛡️ Codex usage guard</span>
            <span className="life-stream-semantic-actions__usage-panel-meta">
              1m {codexUsageGuard.turnsLastMinute} turns ·{" "}
              {Intl.NumberFormat("en-US", { notation: "compact", maximumFractionDigits: 1 }).format(
                codexUsageGuard.tokensLastMinute,
              )}{" "}
              tok
            </span>
          </div>
          <div className="life-stream-semantic-actions__usage-panel-grid">
            <div className="life-stream-semantic-actions__usage-panel-cell">
              <span className="life-stream-semantic-actions__usage-panel-label">5h window</span>
              <span className="life-stream-semantic-actions__usage-panel-value">
                used {formatPercent(codexUsageGuard.primaryUsedPercent)} · left{" "}
                {formatPercent(codexUsageGuard.primaryRemainingPercent)}
              </span>
              <span className="life-stream-semantic-actions__usage-panel-subtle">
                {formatResetLabel(codexUsageGuard.primaryResetsAt)}
              </span>
            </div>
            <div className="life-stream-semantic-actions__usage-panel-cell">
              <span className="life-stream-semantic-actions__usage-panel-label">Weekly</span>
              <span className="life-stream-semantic-actions__usage-panel-value">
                used {formatPercent(codexUsageGuard.secondaryUsedPercent)} · left{" "}
                {formatPercent(codexUsageGuard.secondaryRemainingPercent)}
              </span>
              <span className="life-stream-semantic-actions__usage-panel-subtle">
                {formatResetLabel(codexUsageGuard.secondaryResetsAt)}
              </span>
            </div>
            <div className="life-stream-semantic-actions__usage-panel-cell">
              <span className="life-stream-semantic-actions__usage-panel-label">Live load</span>
              <span className="life-stream-semantic-actions__usage-panel-value">
                {codexUsageGuard.inFlightTurns} in-flight · 5m{" "}
                {codexUsageGuard.turnsLastFiveMinutes} turns ·{" "}
                {Intl.NumberFormat("en-US", { notation: "compact", maximumFractionDigits: 1 }).format(
                  codexUsageGuard.tokensLastFiveMinutes,
                )}{" "}
                tok
              </span>
              <span className="life-stream-semantic-actions__usage-panel-subtle">
                {codexUsageGuard.lastTokenAt
                  ? `last token event ${new Date(codexUsageGuard.lastTokenAt).toLocaleTimeString([], {
                      hour: "numeric",
                      minute: "2-digit",
                      second: "2-digit",
                    })}`
                  : "no token events yet in this session"}
              </span>
            </div>
          </div>
          <div className="life-stream-semantic-actions__usage-panel-footer">
            <span className="life-stream-semantic-actions__usage-panel-alert">
              {codexUsageGuard.message}
            </span>
            <button
              type="button"
              className="life-segment-button life-stream-semantic-actions__button"
              onClick={resetCodexUsageGuardBaseline}
            >
              ♻️ Reset baseline
            </button>
          </div>
        </div>
      </div>
      <EmojiFilters
        activeFilters={activeFilters}
        onToggle={toggleFilter}
        onClear={clearFilters}
      />
    </div>
  );
}
