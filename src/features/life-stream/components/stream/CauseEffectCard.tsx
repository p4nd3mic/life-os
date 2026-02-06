import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import type {
  CardType,
  CausalCardContent,
  CausalNode,
  CausalRestructureAction,
} from "../../types";
import { GraphArrowLayer, type GraphArrowPath } from "./GraphArrowLayer";
import { nodeAnchorId } from "../../utils/anchors";
import "./CauseEffectCard.css";

type CauseEffectCardProps = {
  cardId: string;
  cardType: CardType;
  causal: CausalCardContent;
  onRestructure: (
    action: CausalRestructureAction,
    options?: { sourceNodeIds?: string[]; targetMode?: "cause_effect" | "action_reward" },
  ) => void;
  onRequestNodeImage?: (nodeId: string) => void;
};

const DEFAULT_VISIBLE_RIGHT_COUNT = 3;
const DEFAULT_TOP_LINK_LIMIT = 3;
const ARROW_HEAD_LENGTH = 13;
const ARROW_HEAD_WIDTH = 9;
const IS_JSDOM_ENV =
  typeof navigator !== "undefined" && /jsdom/i.test(navigator.userAgent);

function pathsEqual(previous: GraphArrowPath[], next: GraphArrowPath[]): boolean {
  if (previous.length !== next.length) {
    return false;
  }
  for (let index = 0; index < previous.length; index += 1) {
    const prev = previous[index];
    const current = next[index];
    if (
      prev.id !== current.id ||
      prev.d !== current.d ||
      prev.headD !== current.headD ||
      prev.dimmed !== current.dimmed
    ) {
      return false;
    }
  }
  return true;
}

function laneLabelFromRole(side: "left" | "right", nodes: CausalNode[]): string {
  const role = nodes[0]?.role;
  if (side === "left") {
    if (role === "action") return "🎯 Action";
    if (role === "question") return "❓ Question";
    return "🧩 Cause";
  }

  if (role === "reward") return "🏆 Reward";
  if (role === "response") return "💬 Response";
  return "✨ Effect";
}

function formatRole(role?: CausalNode["role"]) {
  if (!role) return "";
  return role.replace("_", " ").replace(/\b\w/g, (char) => char.toUpperCase());
}

function resolveImageSrc(url?: string) {
  if (!url) return "";
  if (
    url.startsWith("http") ||
    url.startsWith("tauri://") ||
    url.startsWith("asset://") ||
    url.startsWith("codex://")
  ) {
    return url;
  }
  return convertFileSrc(url);
}

function buildArrowHeadPath(
  tipX: number,
  tipY: number,
  controlX: number,
  controlY: number,
) {
  const tangentX = tipX - controlX;
  const tangentY = tipY - controlY;
  const angle = Math.atan2(tangentY, tangentX);
  const baseX = tipX - Math.cos(angle) * ARROW_HEAD_LENGTH;
  const baseY = tipY - Math.sin(angle) * ARROW_HEAD_LENGTH;
  const side = ARROW_HEAD_WIDTH / 2;

  const leftX = baseX + Math.cos(angle + Math.PI / 2) * side;
  const leftY = baseY + Math.sin(angle + Math.PI / 2) * side;
  const rightX = baseX + Math.cos(angle - Math.PI / 2) * side;
  const rightY = baseY + Math.sin(angle - Math.PI / 2) * side;
  const notchX = tipX - Math.cos(angle) * (ARROW_HEAD_LENGTH * 0.62);
  const notchY = tipY - Math.sin(angle) * (ARROW_HEAD_LENGTH * 0.62);

  return `M ${tipX} ${tipY} L ${leftX} ${leftY} Q ${notchX} ${notchY} ${rightX} ${rightY} Z`;
}

function coerceCollapsedCount(value?: number | null): 2 | 3 {
  if (value === 2) {
    return 2;
  }
  return 3;
}

function resolveMostRecentDeliveryIndex(nodes: CausalNode[]): number {
  let bestIndex = nodes.length - 1;
  let bestTime = Number.NEGATIVE_INFINITY;

  nodes.forEach((node, index) => {
    if (!node.occurredAt) return;
    const parsed = Date.parse(node.occurredAt);
    if (Number.isNaN(parsed)) return;
    if (parsed >= bestTime) {
      bestTime = parsed;
      bestIndex = index;
    }
  });

  return Math.max(0, bestIndex);
}

function NodeCard({
  node,
  side,
  index,
  setRef,
  onRequestNodeImage,
  onPreviewImage,
}: {
  node: CausalNode;
  side: "left" | "right";
  index: number;
  setRef: (nodeId: string, element: HTMLDivElement | null) => void;
  onRequestNodeImage?: (nodeId: string) => void;
  onPreviewImage?: (imageSrc: string, alt: string) => void;
}) {
  const image = node.image;
  const hasImage = image?.status === "ready" && Boolean(image?.url);
  const imageSrc = useMemo(() => resolveImageSrc(image?.url), [image?.url]);
  const roleLabel = formatRole(node.role);
  const roleClass = node.role ? `is-role-${node.role}` : "";
  const isTopRight = side === "right" && index < 3;

  return (
    <div
      ref={(element) => setRef(node.id, element)}
      id={nodeAnchorId(node.id)}
      className={[
        "life-causal-card__node",
        `life-causal-card__node--${side}`,
        roleClass,
        isTopRight ? "is-top-right" : "",
      ]
        .filter(Boolean)
        .join(" ")}
      data-node-id={node.id}
    >
      {side === "right" && (
        <span className="life-causal-card__node-rank" aria-hidden="true">
          {index + 1}
        </span>
      )}
      {(hasImage || roleLabel) && (
        <div className="life-causal-card__node-meta">
          {roleLabel && (
            <span className="life-causal-card__role" aria-label={`Role ${roleLabel}`}>
              {roleLabel}
            </span>
          )}
        </div>
      )}
      {hasImage && (
        <button
          type="button"
          className="life-causal-card__node-image-button"
          onClick={() => onPreviewImage?.(imageSrc, node.text)}
          data-no-toggle
        >
          <img
            className="life-causal-card__node-image"
            src={imageSrc}
            alt={node.text}
            loading="lazy"
          />
          <span className="life-causal-card__node-image-zoom">🔍 Expand</span>
        </button>
      )}
      <div className="life-causal-card__node-text">{node.text}</div>
      {image?.status === "missing" && (
        <div className="life-causal-card__node-status">
          Missing image
          {onRequestNodeImage && (
            <button
              type="button"
              className="life-causal-card__node-image-action"
              onClick={() => onRequestNodeImage(node.id)}
              data-no-toggle
            >
              📷 Set image
            </button>
          )}
        </div>
      )}
      {image?.status === "upload_prompt" && (
        <div className="life-causal-card__node-status">
          Add image later
          {onRequestNodeImage && (
            <button
              type="button"
              className="life-causal-card__node-image-action"
              onClick={() => onRequestNodeImage(node.id)}
              data-no-toggle
            >
              📷 Set image
            </button>
          )}
        </div>
      )}
      {!hasImage && !image?.status && onRequestNodeImage && (
        <div className="life-causal-card__node-status">
          No image yet
          <button
            type="button"
            className="life-causal-card__node-image-action"
            onClick={() => onRequestNodeImage(node.id)}
            data-no-toggle
          >
            📷 Set image
          </button>
        </div>
      )}
    </div>
  );
}

export function CauseEffectCard({
  cardId,
  cardType,
  causal,
  onRestructure,
  onRequestNodeImage,
}: CauseEffectCardProps) {
  const gradientId = useMemo(
    () => `life-causal-link-gradient-${cardId.replace(/[^a-zA-Z0-9_-]/g, "-")}`,
    [cardId],
  );
  const defaultCollapsedCount = coerceCollapsedCount(causal.layout?.visibleRightCount);
  const topLinkLimit =
    causal.layout?.topLinkLimit && causal.layout.topLinkLimit > 0
      ? causal.layout.topLinkLimit
      : DEFAULT_TOP_LINK_LIMIT;
  const isDenseDeliverySession = cardType === "delivery_session";

  const [showAllRightNodes, setShowAllRightNodes] = useState(false);
  const [collapsedCount, setCollapsedCount] = useState<2 | 3>(defaultCollapsedCount);
  const [paths, setPaths] = useState<GraphArrowPath[]>([]);
  const [previewImage, setPreviewImage] = useState<{ src: string; alt: string } | null>(null);
  const containerRef = useRef<HTMLDivElement | null>(null);
  const leftNodeRefs = useRef<Map<string, HTMLDivElement>>(new Map());
  const rightNodeRefs = useRef<Map<string, HTMLDivElement>>(new Map());
  const recomputeRafRef = useRef<number | null>(null);
  const disposedRef = useRef(false);

  useEffect(() => {
    setCollapsedCount(defaultCollapsedCount);
  }, [defaultCollapsedCount, cardId]);

  useEffect(() => {
    setPreviewImage(null);
  }, [cardId]);

  useEffect(() => {
    if (!previewImage) return;

    const handleEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setPreviewImage(null);
      }
    };

    window.addEventListener("keydown", handleEscape);
    return () => window.removeEventListener("keydown", handleEscape);
  }, [previewImage]);

  const visibleRightNodes = useMemo(() => {
    if (showAllRightNodes) {
      return causal.rightNodes;
    }

    const collapseLimit = Math.min(
      Math.max(2, collapsedCount),
      DEFAULT_VISIBLE_RIGHT_COUNT,
    );
    if (causal.rightNodes.length <= collapseLimit) {
      return causal.rightNodes;
    }

    if (!isDenseDeliverySession) {
      return causal.rightNodes.slice(0, collapseLimit);
    }

    const newestIndex = resolveMostRecentDeliveryIndex(causal.rightNodes);
    const start = Math.max(0, newestIndex - collapseLimit + 1);
    return causal.rightNodes.slice(start, newestIndex + 1);
  }, [causal.rightNodes, collapsedCount, isDenseDeliverySession, showAllRightNodes]);

  const visibleRightNodeIds = useMemo(
    () => new Set(visibleRightNodes.map((node) => node.id)),
    [visibleRightNodes],
  );

  const linkCandidates = useMemo(() => {
    return causal.links.filter((link) => visibleRightNodeIds.has(link.toId));
  }, [causal.links, visibleRightNodeIds]);

  const visibleLinks = useMemo(() => {
    if (showAllRightNodes || linkCandidates.length <= topLinkLimit) {
      return linkCandidates;
    }
    return [...linkCandidates]
      .sort((a, b) => (b.strength ?? 1) - (a.strength ?? 1))
      .slice(0, topLinkLimit);
  }, [linkCandidates, showAllRightNodes, topLinkLimit]);

  const registerLeftRef = useCallback((nodeId: string, element: HTMLDivElement | null) => {
    if (element) {
      leftNodeRefs.current.set(nodeId, element);
      return;
    }
    leftNodeRefs.current.delete(nodeId);
  }, []);

  const registerRightRef = useCallback((nodeId: string, element: HTMLDivElement | null) => {
    if (element) {
      rightNodeRefs.current.set(nodeId, element);
      return;
    }
    rightNodeRefs.current.delete(nodeId);
  }, []);

  const recomputeLinkPathsNow = useCallback(() => {
    const container = containerRef.current;
    if (!container) {
      setPaths((previous) => (previous.length === 0 ? previous : []));
      return;
    }

    const containerBounds = container.getBoundingClientRect();
    const nextPaths: GraphArrowPath[] = [];

    for (const link of visibleLinks) {
      const fromElement =
        leftNodeRefs.current.get(link.fromId) ??
        rightNodeRefs.current.get(link.fromId);
      const toElement =
        rightNodeRefs.current.get(link.toId) ??
        leftNodeRefs.current.get(link.toId);

      if (!fromElement || !toElement) {
        continue;
      }

      const fromBounds = fromElement.getBoundingClientRect();
      const toBounds = toElement.getBoundingClientRect();
      const startX = fromBounds.right - containerBounds.left + 0.5;
      const startY = fromBounds.top - containerBounds.top + fromBounds.height / 2;
      const endX = toBounds.left - containerBounds.left - 0.5;
      const endY = toBounds.top - containerBounds.top + toBounds.height / 2;

      const horizontalDistance = Math.max(36, endX - startX);
      const c1x = startX + horizontalDistance * 0.38;
      const c1y = startY;
      const c2x = endX - horizontalDistance * 0.46;
      const c2y = endY;
      const pathD = `M ${startX} ${startY} C ${c1x} ${c1y}, ${c2x} ${c2y}, ${endX} ${endY}`;
      const headD = buildArrowHeadPath(endX, endY, c2x, c2y);

      nextPaths.push({
        id: link.id ?? `${link.fromId}->${link.toId}`,
        d: pathD,
        headD,
        dimmed: false,
      });
    }

    setPaths((previous) => (pathsEqual(previous, nextPaths) ? previous : nextPaths));
  }, [visibleLinks]);

  const scheduleLinkPathRecompute = useCallback(() => {
    if (disposedRef.current) {
      return;
    }
    if (IS_JSDOM_ENV) {
      recomputeLinkPathsNow();
      return;
    }
    if (typeof window === "undefined" || typeof window.requestAnimationFrame !== "function") {
      recomputeLinkPathsNow();
      return;
    }
    if (recomputeRafRef.current !== null) {
      return;
    }
    recomputeRafRef.current = window.requestAnimationFrame(() => {
      recomputeRafRef.current = null;
      if (disposedRef.current) {
        return;
      }
      recomputeLinkPathsNow();
    });
  }, [recomputeLinkPathsNow]);

  useLayoutEffect(() => {
    scheduleLinkPathRecompute();
  }, [scheduleLinkPathRecompute, visibleRightNodes, visibleLinks, causal.leftNodes]);

  useEffect(() => {
    if (IS_JSDOM_ENV) {
      return;
    }
    const handleResize = () => scheduleLinkPathRecompute();
    const scrollParent =
      containerRef.current?.closest(".life-stream-messages") ?? window;
    const handleScroll = () => scheduleLinkPathRecompute();
    window.addEventListener("resize", handleResize);
    window.addEventListener("scroll", handleScroll, { passive: true });
    if (scrollParent instanceof Element) {
      scrollParent.addEventListener("scroll", handleScroll, { passive: true });
    }
    return () => {
      window.removeEventListener("resize", handleResize);
      window.removeEventListener("scroll", handleScroll);
      if (scrollParent instanceof Element) {
        scrollParent.removeEventListener("scroll", handleScroll);
      }
    };
  }, [scheduleLinkPathRecompute]);

  useEffect(() => {
    if (IS_JSDOM_ENV) {
      return;
    }
    if (typeof ResizeObserver === "undefined") {
      return;
    }

    const observer = new ResizeObserver(() => {
      scheduleLinkPathRecompute();
    });

    const container = containerRef.current;
    if (container) {
      observer.observe(container);
    }

    for (const element of leftNodeRefs.current.values()) {
      observer.observe(element);
    }
    for (const element of rightNodeRefs.current.values()) {
      observer.observe(element);
    }

    return () => observer.disconnect();
  }, [causal.leftNodes, scheduleLinkPathRecompute, visibleLinks, visibleRightNodes]);

  useEffect(() => {
    if (IS_JSDOM_ENV) {
      return;
    }
    const fonts = (document as Document & { fonts?: { ready?: Promise<unknown> } }).fonts;
    if (!fonts?.ready) {
      return;
    }

    let cancelled = false;
    void fonts.ready.then(() => {
      if (!cancelled) {
        scheduleLinkPathRecompute();
      }
    });

    return () => {
      cancelled = true;
    };
  }, [scheduleLinkPathRecompute]);

  useEffect(() => {
    disposedRef.current = false;
    return () => {
      disposedRef.current = true;
      if (recomputeRafRef.current !== null) {
        if (typeof window !== "undefined" && typeof window.cancelAnimationFrame === "function") {
          window.cancelAnimationFrame(recomputeRafRef.current);
        }
        recomputeRafRef.current = null;
      }
    };
  }, []);

  const hasHiddenRightNodes = causal.rightNodes.length > collapsedCount;
  const leftLaneLabel = laneLabelFromRole("left", causal.leftNodes);
  const rightLaneLabel = laneLabelFromRole("right", causal.rightNodes);
  const canToggleDensity = isDenseDeliverySession && causal.rightNodes.length >= 3;

  const handleSplitCause = useCallback(() => {
    onRestructure("split_cause");
  }, [onRestructure]);

  const handleMergeEffects = useCallback(() => {
    onRestructure("merge_effects");
  }, [onRestructure]);

  const handleRelink = useCallback(() => {
    onRestructure("relink_arrows");
  }, [onRestructure]);

  const handleReframe = useCallback(() => {
    onRestructure("reframe_mode");
  }, [onRestructure]);

  const canSplitCause = causal.leftNodes.length === 1;
  const canMergeEffects = causal.rightNodes.length >= 2;

  return (
    <section className="life-causal-card" data-card-id={cardId}>
      <div className="life-causal-card__labels">
        <div className="life-causal-card__lane-label life-causal-card__lane-label--left">
          {leftLaneLabel}
        </div>
        <div className="life-causal-card__lane-label life-causal-card__lane-label--right">
          {rightLaneLabel}
        </div>
      </div>

      <div ref={containerRef} className="life-causal-card__lanes">
        <div className="life-causal-card__lane life-causal-card__lane--left">
          {causal.leftNodes.map((node, index) => (
            <NodeCard
              key={node.id}
              node={node}
              side="left"
              index={index}
              setRef={registerLeftRef}
              onRequestNodeImage={onRequestNodeImage}
              onPreviewImage={(src, alt) => setPreviewImage({ src, alt })}
            />
          ))}
        </div>

        <GraphArrowLayer paths={paths} gradientId={gradientId} />

        <div className="life-causal-card__lane life-causal-card__lane--right">
          {visibleRightNodes.map((node, index) => (
            <NodeCard
              key={node.id}
              node={node}
              side="right"
              index={index}
              setRef={registerRightRef}
              onRequestNodeImage={onRequestNodeImage}
              onPreviewImage={(src, alt) => setPreviewImage({ src, alt })}
            />
          ))}
        </div>
      </div>

      {(hasHiddenRightNodes || canToggleDensity) && (
        <div className="life-causal-card__controls" data-no-toggle>
          {canToggleDensity && !showAllRightNodes && (
            <button
              type="button"
              className="life-causal-card__control"
              onClick={() => setCollapsedCount((prev) => (prev === 3 ? 2 : 3))}
            >
              Latest {collapsedCount} (toggle 2/3)
            </button>
          )}
          {hasHiddenRightNodes && (
            <button
              type="button"
              className="life-causal-card__control"
              onClick={() => setShowAllRightNodes((prev) => !prev)}
            >
              {showAllRightNodes
                ? `Show latest ${collapsedCount}`
                : `Show all outcomes (${causal.rightNodes.length})`}
            </button>
          )}
          {!showAllRightNodes && linkCandidates.length > visibleLinks.length && (
            <span className="life-causal-card__control-hint">
              Top links shown ({visibleLinks.length}/{linkCandidates.length})
            </span>
          )}
        </div>
      )}

      <div className="life-causal-card__actions" data-no-toggle>
        <button
          type="button"
          className="life-causal-card__action"
          onClick={handleSplitCause}
          disabled={!canSplitCause}
        >
          ✂️ Split cause
        </button>
        <button
          type="button"
          className="life-causal-card__action"
          onClick={handleMergeEffects}
          disabled={!canMergeEffects}
        >
          🧩 Merge effects
        </button>
        <button
          type="button"
          className="life-causal-card__action"
          onClick={handleRelink}
        >
          🔗 Re-link arrows
        </button>
        <button
          type="button"
          className="life-causal-card__action"
          onClick={handleReframe}
        >
          🔁 Reframe mode
        </button>
      </div>

      {previewImage && (
        <div
          className="life-causal-card__lightbox"
          role="dialog"
          aria-label="Expanded image preview"
          data-no-toggle
          onClick={() => setPreviewImage(null)}
        >
          <div
            className="life-causal-card__lightbox-content"
            onClick={(event) => event.stopPropagation()}
          >
            <button
              type="button"
              className="life-causal-card__lightbox-close"
              onClick={() => setPreviewImage(null)}
              aria-label="Close image preview"
            >
              ✕
            </button>
            <img
              src={previewImage.src}
              alt={previewImage.alt}
              className="life-causal-card__lightbox-image"
            />
          </div>
        </div>
      )}
    </section>
  );
}
