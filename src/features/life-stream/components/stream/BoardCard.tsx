import {
  useCallback,
  useEffect,
  useRef,
  type KeyboardEvent as ReactKeyboardEvent,
  type MouseEvent as ReactMouseEvent,
  type PointerEvent as ReactPointerEvent,
} from "react";
import type { CausalNode, ImageStatus } from "../../types";
import { nodeAnchorId } from "../../utils/anchors";
import "./BoardCard.css";

export type BoardCardColorVariant = "gold" | "green" | "blue";

export type BoardCardResolvedNode = {
  leadLine: string;
  bullets: string[];
  imageSrc?: string;
  imageStatus: ImageStatus;
  ariaTitle: string;
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
  onRequestImage?: (nodeId: string) => void;
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
  onRequestImage,
}: BoardCardProps) {
  const longPressRef = useRef<number | null>(null);
  const didLongPressRef = useRef(false);
  const longPressPointRef = useRef<{ x: number; y: number } | null>(null);
  const hasImage = resolved.imageStatus === "ready" && Boolean(resolved.imageSrc);
  const ariaTitle = resolved.ariaTitle || resolved.leadLine || `Card ${rank}`;
  const imageStatusLabel =
    resolved.imageStatus === "loading"
      ? "Image is loading"
      : resolved.imageStatus === "upload_prompt"
        ? "Image slot ready"
        : "No image yet";

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
      aria-label={`Card ${rank}: ${ariaTitle}`}
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

      <div className="board-card__art-shell" data-image-state={resolved.imageStatus}>
        {hasImage ? (
          <img
            src={resolved.imageSrc}
            alt={ariaTitle}
            className="board-card__art-image"
            loading="lazy"
          />
        ) : (
          <div className="board-card__art-placeholder" data-no-toggle>
            <span className="board-card__art-placeholder-icon" aria-hidden="true">
              🖼️
            </span>
            <span className="board-card__art-placeholder-title">{imageStatusLabel}</span>
            <span className="board-card__art-placeholder-copy">
              Add card art to reinforce context at a glance.
            </span>
            {onRequestImage && (
              <button
                type="button"
                className="board-card__art-action"
                onClick={(event) => {
                  event.stopPropagation();
                  onRequestImage(node.id);
                }}
                data-no-toggle
              >
                📷 Fetch image
              </button>
            )}
          </div>
        )}
      </div>

      {resolved.leadLine && <h3 className="board-card__lead">{resolved.leadLine}</h3>}

      <div className="board-card__divider" aria-hidden="true" />

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
