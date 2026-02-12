import {
  useCallback,
  useEffect,
  useRef,
  type KeyboardEvent as ReactKeyboardEvent,
  type MouseEvent as ReactMouseEvent,
  type PointerEvent as ReactPointerEvent,
} from "react";
import type { CausalNode } from "../../types";
import { nodeAnchorId } from "../../utils/anchors";
import "./BoardCard.css";

export type BoardCardColorVariant = "gold" | "green" | "blue";

export type BoardCardResolvedNode = {
  title: string;
  summaryLine: string;
  bullets: string[];
};

type BoardCardProps = {
  node: CausalNode;
  rank: number;
  colorVariant: BoardCardColorVariant;
  resolved: BoardCardResolvedNode;
  isSelected: boolean;
  setRef: (nodeId: string, element: HTMLDivElement | null) => void;
  onSelect: (nodeId: string, options?: { toggle?: boolean }) => void;
  onOpenContextMenu: (nodeId: string, point: { x: number; y: number }) => void;
};

export function BoardCard({
  node,
  rank,
  colorVariant,
  resolved,
  isSelected,
  setRef,
  onSelect,
  onOpenContextMenu,
}: BoardCardProps) {
  const longPressRef = useRef<number | null>(null);
  const didLongPressRef = useRef(false);
  const longPressPointRef = useRef<{ x: number; y: number } | null>(null);

  const clearLongPress = useCallback(() => {
    if (longPressRef.current !== null) {
      window.clearTimeout(longPressRef.current);
      longPressRef.current = null;
    }
    longPressPointRef.current = null;
  }, []);

  useEffect(() => {
    return () => clearLongPress();
  }, [clearLongPress]);

  const handlePointerDown = useCallback(
    (event: ReactPointerEvent<HTMLDivElement>) => {
      if (event.pointerType !== "touch") {
        return;
      }
      clearLongPress();
      didLongPressRef.current = false;
      const point = { x: event.clientX, y: event.clientY };
      longPressPointRef.current = point;
      longPressRef.current = window.setTimeout(() => {
        didLongPressRef.current = true;
        onSelect(node.id, { toggle: false });
        onOpenContextMenu(node.id, longPressPointRef.current ?? point);
        longPressRef.current = null;
      }, 320);
    },
    [clearLongPress, node.id, onOpenContextMenu, onSelect],
  );

  const handleClick = useCallback(() => {
    if (didLongPressRef.current) {
      didLongPressRef.current = false;
      return;
    }
    onSelect(node.id, { toggle: true });
  }, [node.id, onSelect]);

  const handleContextMenu = useCallback(
    (event: ReactMouseEvent<HTMLDivElement>) => {
      event.preventDefault();
      onSelect(node.id, { toggle: false });
      onOpenContextMenu(node.id, { x: event.clientX, y: event.clientY });
    },
    [node.id, onOpenContextMenu, onSelect],
  );

  const handleKeyDown = useCallback(
    (event: ReactKeyboardEvent<HTMLDivElement>) => {
      if (event.key === "Enter" && event.ctrlKey) {
        event.preventDefault();
        const bounds = event.currentTarget.getBoundingClientRect();
        onSelect(node.id, { toggle: false });
        onOpenContextMenu(node.id, {
          x: bounds.left + bounds.width / 2,
          y: bounds.top + Math.min(42, bounds.height / 2),
        });
        return;
      }

      if (event.key === "Enter" || event.key === " ") {
        event.preventDefault();
        onSelect(node.id, { toggle: true });
        return;
      }

      if (event.key === "ContextMenu" || (event.shiftKey && event.key === "F10")) {
        event.preventDefault();
        const bounds = event.currentTarget.getBoundingClientRect();
        onSelect(node.id, { toggle: false });
        onOpenContextMenu(node.id, {
          x: bounds.left + bounds.width / 2,
          y: bounds.top + Math.min(42, bounds.height / 2),
        });
      }
    },
    [node.id, onOpenContextMenu, onSelect],
  );

  return (
    <div
      ref={(element) => setRef(node.id, element)}
      id={nodeAnchorId(node.id)}
      data-node-id={node.id}
      data-node-rank={rank}
      tabIndex={0}
      role="button"
      aria-label={`Card ${rank}: ${resolved.title}`}
      className={[
        "board-card",
        `board-card--${colorVariant}`,
        isSelected ? "is-selected" : "",
      ]
        .filter(Boolean)
        .join(" ")}
      onPointerDown={handlePointerDown}
      onPointerUp={clearLongPress}
      onPointerLeave={clearLongPress}
      onPointerCancel={clearLongPress}
      onClick={handleClick}
      onContextMenu={handleContextMenu}
      onKeyDown={handleKeyDown}
    >
      <div className="board-card__rank" aria-hidden="true">
        {rank}
      </div>

      <h3 className="board-card__title">{resolved.title}</h3>

      {resolved.summaryLine && (
        <p className="board-card__summary">{resolved.summaryLine}</p>
      )}

      {resolved.bullets.length > 0 && (
        <ol className="board-card__bullets" data-no-toggle>
          {resolved.bullets.map((bullet, index) => (
            <li key={`${node.id}:board-bullet:${index}`}>
              <span className="board-card__bullet-num" aria-hidden="true">
                {index + 1}
              </span>
              <span className="board-card__bullet-text">{bullet}</span>
            </li>
          ))}
        </ol>
      )}
    </div>
  );
}
