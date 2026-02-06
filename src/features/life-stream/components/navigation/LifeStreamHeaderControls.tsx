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
    imageAutoFetchStatus,
    autoFetchImagesForCurrentDate,
    activeFilters,
    toggleFilter,
    clearFilters,
  } = context;

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
      </div>
      <EmojiFilters
        activeFilters={activeFilters}
        onToggle={toggleFilter}
        onClear={clearFilters}
      />
    </div>
  );
}
