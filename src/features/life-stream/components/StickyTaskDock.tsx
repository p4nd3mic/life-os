import { FormEvent, useCallback, useMemo, useState } from "react";
import { useLifeStreamContext } from "../context/LifeStreamContext";
import type { TaskDockItem } from "../types";
import "./StickyTaskDock.css";

type StickyTaskDockProps = {
  onJumpToSource: (item: TaskDockItem) => void;
  onReviewCandidates?: (item: TaskDockItem) => void;
};

function isReviewCandidateTask(item: TaskDockItem): boolean {
  return item.key.startsWith("review-image-candidates:");
}

export function StickyTaskDock({ onJumpToSource, onReviewCandidates }: StickyTaskDockProps) {
  const {
    currentDate,
    taskDock,
    addTaskDockItem,
    toggleTaskDockItem,
    moveTaskDockItem,
    setTaskDockFilter,
    setTaskDockCollapsed,
  } = useLifeStreamContext();
  const [draft, setDraft] = useState("");

  const visibleItems = useMemo(() => {
    if (taskDock.filter === "all") {
      return taskDock.items;
    }
    return taskDock.items.filter((item) => item.targetDate === currentDate);
  }, [currentDate, taskDock.filter, taskDock.items]);

  const openCount = useMemo(
    () => visibleItems.filter((item) => !item.completed).length,
    [visibleItems],
  );

  const handleSubmit = useCallback(
    (event: FormEvent<HTMLFormElement>) => {
      event.preventDefault();
      const text = draft.trim();
      if (!text) return;
      addTaskDockItem(text);
      setDraft("");
    },
    [addTaskDockItem, draft],
  );

  if (taskDock.hidden) {
    return null;
  }

  return (
    <section
      className={`life-sticky-task-dock${taskDock.collapsed ? " is-collapsed" : ""}`}
      data-no-toggle
      aria-label="Sticky tasks and reminders"
    >
      <header className="life-sticky-task-dock__header">
        <div className="life-sticky-task-dock__title-wrap">
          <div className="life-sticky-task-dock__title">📌 Next actions</div>
          <div className="life-sticky-task-dock__meta">
            {openCount} open • {visibleItems.length} shown
          </div>
        </div>

        <div className="life-sticky-task-dock__actions">
          <button
            type="button"
            className={`life-sticky-task-dock__chip${
              taskDock.filter === "today" ? " is-active" : ""
            }`}
            onClick={() => setTaskDockFilter("today")}
          >
            Today
          </button>
          <button
            type="button"
            className={`life-sticky-task-dock__chip${
              taskDock.filter === "all" ? " is-active" : ""
            }`}
            onClick={() => setTaskDockFilter("all")}
          >
            Backlog
          </button>
          <button
            type="button"
            className="life-sticky-task-dock__collapse"
            onClick={() => setTaskDockCollapsed(!taskDock.collapsed)}
            aria-expanded={!taskDock.collapsed}
          >
            {taskDock.collapsed ? "▲" : "▼"}
          </button>
        </div>
      </header>

      {!taskDock.collapsed && (
        <>
          <form className="life-sticky-task-dock__composer" onSubmit={handleSubmit}>
            <input
              type="text"
              value={draft}
              onChange={(event) => setDraft(event.target.value)}
              placeholder="Add a task/reminder..."
              aria-label="Add task or reminder"
            />
            <button type="submit">Add</button>
          </form>

          <ul className="life-sticky-task-dock__list">
            {visibleItems.length === 0 && (
              <li className="life-sticky-task-dock__empty">
                ✅ Nothing queued. Add a task above.
              </li>
            )}
            {visibleItems.map((item, index) => (
              <li
                key={item.id}
                className={`life-sticky-task-dock__item${item.completed ? " is-complete" : ""}`}
              >
                <button
                  type="button"
                  className="life-sticky-task-dock__check"
                  onClick={() => toggleTaskDockItem(item.id)}
                >
                  {item.completed ? "✅" : "⬜️"}
                </button>

                <button
                  type="button"
                  className="life-sticky-task-dock__text"
                  onClick={() => {
                    if (item.sourceCardId || item.sourceNodeId) {
                      onJumpToSource(item);
                    } else {
                      toggleTaskDockItem(item.id);
                    }
                  }}
                >
                  {item.kind === "reminder" ? "🔔 " : ""}
                  {item.text}
                </button>

                <div className="life-sticky-task-dock__item-actions">
                  {isReviewCandidateTask(item) && onReviewCandidates && (
                    <button
                      type="button"
                      className="life-sticky-task-dock__review"
                      onClick={() => onReviewCandidates(item)}
                      aria-label="Review image candidates"
                    >
                      Review
                    </button>
                  )}
                  <button
                    type="button"
                    onClick={() => moveTaskDockItem(item.id, -1, visibleItems.map((it) => it.id))}
                    disabled={index === 0}
                    aria-label="Move up"
                  >
                    ↑
                  </button>
                  <button
                    type="button"
                    onClick={() => moveTaskDockItem(item.id, 1, visibleItems.map((it) => it.id))}
                    disabled={index === visibleItems.length - 1}
                    aria-label="Move down"
                  >
                    ↓
                  </button>
                </div>
              </li>
            ))}
          </ul>
        </>
      )}
    </section>
  );
}
