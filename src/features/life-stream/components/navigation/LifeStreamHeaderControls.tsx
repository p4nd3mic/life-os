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
      <EmojiFilters
        activeFilters={activeFilters}
        onToggle={toggleFilter}
        onClear={clearFilters}
      />
    </div>
  );
}
