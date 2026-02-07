import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
} from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import type {
  CardType,
  CausalCardContent,
  CausalNode,
  CausalSemanticMode,
  CausalRestructureAction,
} from "../../types";
import { GraphArrowLayer, type GraphArrowPath } from "./GraphArrowLayer";
import { nodeAnchorId } from "../../utils/anchors";
import "./CauseEffectCard.css";

type CauseEffectCardProps = {
  cardId: string;
  cardType: CardType;
  cardTitle?: string;
  causal: CausalCardContent;
  layoutOrientation?: "horizontal" | "vertical";
  onOpenTranscript?: () => void;
  onRestructure: (
    action: CausalRestructureAction,
    options?: { sourceNodeIds?: string[]; targetMode?: "cause_effect" | "action_reward" },
  ) => void;
  onRequestNodeImage?: (nodeId: string) => void;
};

const TOP_VISIBLE_RIGHT_NODES = 3;
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
      prev.dimmed !== current.dimmed ||
      prev.isTrunk !== current.isTrunk
    ) {
      return false;
    }
  }
  return true;
}

function inferSemanticModeFromNodes(nodes: CausalNode[]): CausalSemanticMode {
  const leftRole = nodes.find((node) => node.role)?.role;
  if (leftRole === "action") return "action_reward";
  if (leftRole === "question") return "question_response";
  if (leftRole === "cause") return "cause_effect";
  return "statement_why";
}

function laneLabel(side: "left" | "right", mode: CausalSemanticMode): string {
  if (mode === "action_reward") {
    return side === "left" ? "🎯 ACTION" : "🏆 REWARD";
  }
  if (mode === "statement_why") {
    return side === "left" ? "🧩 STATEMENT" : "💡 WHY";
  }
  if (mode === "question_response") {
    return side === "left" ? "❓ QUESTION" : "💬 RESPONSE";
  }
  return side === "left" ? "🧩 CAUSE" : "✨ EFFECT";
}

function overflowTitle(mode: CausalSemanticMode): string {
  if (mode === "statement_why") return "More reasons";
  if (mode === "action_reward") return "More outcomes";
  if (mode === "question_response") return "More responses";
  return "More effects";
}

function normalizeSemantic(value: string): string {
  return value
    .toLowerCase()
    .replace(/[’']/g, "")
    .replace(/[^\w\s]/g, " ")
    .replace(/\s+/g, " ")
    .trim();
}

function isGenericNodeTitle(value?: string | null) {
  if (!value) return false;
  const normalized = normalizeSemantic(value);
  return [
    "response",
    "cause",
    "effect",
    "action",
    "reward",
    "question",
    "thought",
    "note",
    "codex response",
    "details",
  ].includes(normalized);
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

function resolveNodeTitle(node: CausalNode, fallbackTitle?: string): string {
  const headline = node.headline?.trim();
  if (headline && !isGenericNodeTitle(headline)) {
    return headline;
  }

  const explicitTitle = node.title?.trim();
  if (explicitTitle && !isGenericNodeTitle(explicitTitle)) {
    return explicitTitle;
  }

  const fallback = fallbackTitle?.trim();
  if (fallback && !isGenericNodeTitle(fallback)) {
    return fallback;
  }

  const bulletCandidate = node.bullets
    ?.map((item) => item.trim())
    .find((item) => item.length > 3 && !isGenericNodeTitle(item));
  if (bulletCandidate) {
    return bulletCandidate.length > 120
      ? `${bulletCandidate.slice(0, 120)}...`
      : bulletCandidate;
  }

  const sourceText = (node.details ?? node.text).trim();
  const sentenceCandidate = sourceText
    .split(/[\n.!?]/)
    .map((item) => item.trim())
    .find((item) => item.length > 3 && !isGenericNodeTitle(item));

  if (sentenceCandidate) {
    return sentenceCandidate.length > 120
      ? `${sentenceCandidate.slice(0, 120)}...`
      : sentenceCandidate;
  }

  if (sourceText && !isGenericNodeTitle(sourceText)) {
    return sourceText.length > 120
      ? `${sourceText.slice(0, 120)}...`
      : sourceText;
  }

  return "Details";
}

function resolveSummaryLine(
  node: CausalNode,
  normalizedTitle: string,
  options: { suppress: boolean },
): string {
  if (options.suppress) {
    return "";
  }

  const explicitSummary = node.summaryLine?.trim();
  if (explicitSummary) {
    const normalizedSummary = normalizeSemantic(explicitSummary);
    if (normalizedSummary !== normalizedTitle) {
      return explicitSummary;
    }
  }

  const firstBullet = node.bullets
    ?.map((item) => item.trim())
    .find((item) => {
      if (item.length < 14) return false;
      return normalizeSemantic(item) !== normalizedTitle;
    });
  if (firstBullet) {
    return firstBullet.length > 170 ? `${firstBullet.slice(0, 170)}...` : firstBullet;
  }

  const source = (node.details ?? node.text).trim();
  const sentence = source
    .split(/[\n.!?]/)
    .map((item) => item.trim())
    .find((item) => item.length >= 16);
  if (!sentence) {
    return "";
  }

  const normalizedSentence = normalizeSemantic(sentence);
  if (normalizedSentence === normalizedTitle) {
    return "";
  }

  return sentence.length > 180 ? `${sentence.slice(0, 180)}...` : sentence;
}

function resolveOverflowLines(node: CausalNode): string[] {
  const fromBullets = (node.bullets ?? [])
    .map((line) => line.trim())
    .map((line) => line.replace(/^\s*(?:[-*•]|\d+[.)])\s+/, ""))
    .filter((line) => line.length > 0);
  if (fromBullets.length > 0) {
    return fromBullets;
  }
  return node.text
    .split("\n")
    .map((line) => line.trim())
    .map((line) => line.replace(/^\s*(?:[-*•]|\d+[.)])\s+/, ""))
    .filter((line) => line.length > 0);
}

function resolveRightNodeBullets(
  node: CausalNode,
  normalizedTitle: string,
): string[] {
  const fromBullets = (node.bullets ?? [])
    .map((line) => line.trim())
    .map((line) => line.replace(/^\s*(?:[-*•]|\d+[.)])\s+/, ""))
    .filter((line) => line.length > 0)
    .filter((line) => normalizeSemantic(line) !== normalizedTitle);
  if (fromBullets.length > 0) {
    return fromBullets;
  }

  const detailsSource = (node.details ?? "").trim();
  if (!detailsSource) {
    return [];
  }

  const fromDetails = detailsSource
    .split("\n")
    .map((line) => line.trim())
    .map((line) => line.replace(/^\s*(?:[-*•]|\d+[.)])\s+/, ""))
    .filter((line) => line.length > 0)
    .filter((line) => normalizeSemantic(line) !== normalizedTitle);

  return fromDetails;
}

function NodeCard({
  node,
  side,
  index,
  layoutOrientation,
  staggerOffsetPx,
  fallbackTitle,
  selected,
  setRef,
  onSelectNode,
  onRequestNodeImage,
  onPreviewImage,
  onOpenTranscript,
}: {
  node: CausalNode;
  side: "left" | "right";
  index: number;
  fallbackTitle?: string;
  selected: boolean;
  setRef: (nodeId: string, element: HTMLDivElement | null) => void;
  onSelectNode: (nodeId: string) => void;
  onRequestNodeImage?: (nodeId: string) => void;
  onPreviewImage?: (imageSrc: string, alt: string) => void;
  onOpenTranscript?: () => void;
  layoutOrientation: "horizontal" | "vertical";
  staggerOffsetPx?: number;
}) {
  const longPressRef = useRef<number | null>(null);
  const didLongPressRef = useRef(false);
  const isOverflowSummary = node.groupType === "overflow_summary";
  const isLeftStatement = side === "left";

  const image = node.image;
  const hasImage = image?.status === "ready" && Boolean(image?.url);
  const imageSrc = useMemo(() => resolveImageSrc(image?.url), [image?.url]);

  const nodeTitle = useMemo(() => resolveNodeTitle(node, fallbackTitle), [fallbackTitle, node]);
  const normalizedTitle = useMemo(() => normalizeSemantic(nodeTitle), [nodeTitle]);

  const summaryLine = useMemo(
    () =>
      resolveSummaryLine(node, normalizedTitle, {
        suppress: isLeftStatement,
      }),
    [isLeftStatement, node, normalizedTitle],
  );

  const overflowLines = useMemo(() => resolveOverflowLines(node), [node]);
  const rightDetailBullets = useMemo(
    () => (isLeftStatement || isOverflowSummary ? [] : resolveRightNodeBullets(node, normalizedTitle)),
    [isLeftStatement, isOverflowSummary, node, normalizedTitle],
  );
  const overflowStart = useMemo(
    () => Math.max(1, node.rank ?? TOP_VISIBLE_RIGHT_NODES + 1),
    [node.rank],
  );

  const shouldShowImageStatus = useMemo(() => {
    if (isOverflowSummary) return false;
    if (isLeftStatement) return true;
    return node.isImageApplicable === true && Boolean(node.image?.status);
  }, [isLeftStatement, isOverflowSummary, node.image?.status, node.isImageApplicable]);

  const clearLongPress = useCallback(() => {
    if (longPressRef.current !== null) {
      window.clearTimeout(longPressRef.current);
      longPressRef.current = null;
    }
  }, []);

  useEffect(() => {
    return () => clearLongPress();
  }, [clearLongPress]);

  const handlePointerDown = useCallback(
    (event: React.PointerEvent<HTMLDivElement>) => {
      if (event.pointerType !== "touch" || isOverflowSummary) {
        return;
      }
      clearLongPress();
      didLongPressRef.current = false;
      longPressRef.current = window.setTimeout(() => {
        didLongPressRef.current = true;
        onSelectNode(node.id);
        longPressRef.current = null;
      }, 320);
    },
    [clearLongPress, isOverflowSummary, node.id, onSelectNode],
  );

  const handleClick = useCallback(() => {
    if (didLongPressRef.current) {
      didLongPressRef.current = false;
      return;
    }
    if (isOverflowSummary) {
      return;
    }
    onSelectNode(node.id);
    if (isLeftStatement) {
      onOpenTranscript?.();
    }
  }, [isLeftStatement, isOverflowSummary, node.id, onOpenTranscript, onSelectNode]);

  return (
    <div
      ref={(element) => setRef(node.id, element)}
      id={nodeAnchorId(node.id)}
      className={[
        "life-causal-card__node",
        `life-causal-card__node--${side}`,
        selected ? "is-selected" : "",
        isOverflowSummary ? "is-overflow-summary" : "",
      ]
        .filter(Boolean)
        .join(" ")}
      data-node-id={node.id}
      style={
        {
          "--life-node-stagger-y":
            side === "right" && layoutOrientation === "vertical"
              ? `${staggerOffsetPx ?? 0}px`
              : "0px",
        } as CSSProperties
      }
      onPointerDown={handlePointerDown}
      onPointerUp={clearLongPress}
      onPointerLeave={clearLongPress}
      onClick={handleClick}
    >
      {side === "right" && !isOverflowSummary && (
        <span className="life-causal-card__node-rank" aria-hidden="true">
          {node.rank ?? index + 1}
        </span>
      )}

      <div className="life-causal-card__node-title">{nodeTitle}</div>

      {hasImage && (
        <button
          type="button"
          className="life-causal-card__node-image-button"
          onClick={(event) => {
            event.stopPropagation();
            onPreviewImage?.(imageSrc, nodeTitle);
          }}
          data-no-toggle
        >
          <img
            className="life-causal-card__node-image"
            src={imageSrc}
            alt={nodeTitle}
            loading="lazy"
          />
          <span className="life-causal-card__node-image-zoom">🔍 Expand</span>
        </button>
      )}

      {!isOverflowSummary && summaryLine && (
        <div className="life-causal-card__node-summary-line">{summaryLine}</div>
      )}

      {isOverflowSummary && (
        <ul
          className="life-causal-card__overflow-list"
          data-no-toggle
        >
          {overflowLines.map((line, lineIndex) => {
            const number = overflowStart + lineIndex;
            return (
              <li key={`${node.id}:overflow:${lineIndex}`}>
                <span className="life-causal-card__overflow-number" aria-hidden="true">
                  {number}.
                </span>
                <span className="life-causal-card__overflow-text">{line}</span>
              </li>
            );
          })}
        </ul>
      )}

      {!isOverflowSummary && !isLeftStatement && rightDetailBullets.length > 0 && (
        <ul className="life-causal-card__node-bullets" data-no-toggle>
          {rightDetailBullets.map((line, lineIndex) => (
            <li key={`${node.id}:detail:${lineIndex}`}>{line}</li>
          ))}
        </ul>
      )}

      {shouldShowImageStatus && image?.status === "missing" && (
        <div className="life-causal-card__node-status">
          Missing image
          {onRequestNodeImage && (
            <button
              type="button"
              className="life-causal-card__node-image-action"
              onClick={(event) => {
                event.stopPropagation();
                onRequestNodeImage(node.id);
              }}
              data-no-toggle
            >
              📷 Set image
            </button>
          )}
        </div>
      )}

      {shouldShowImageStatus && image?.status === "upload_prompt" && (
        <div className="life-causal-card__node-status">
          Add image later
          {onRequestNodeImage && (
            <button
              type="button"
              className="life-causal-card__node-image-action"
              onClick={(event) => {
                event.stopPropagation();
                onRequestNodeImage(node.id);
              }}
              data-no-toggle
            >
              📷 Set image
            </button>
          )}
        </div>
      )}

      {shouldShowImageStatus && !hasImage && !image?.status && isLeftStatement && onRequestNodeImage && (
        <div className="life-causal-card__node-status">
          No image yet
          <button
            type="button"
            className="life-causal-card__node-image-action"
            onClick={(event) => {
              event.stopPropagation();
              onRequestNodeImage(node.id);
            }}
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
  cardTitle,
  causal,
  layoutOrientation = "vertical",
  onOpenTranscript,
  onRestructure,
  onRequestNodeImage,
}: CauseEffectCardProps) {
  const gradientId = useMemo(
    () => `life-causal-link-gradient-${cardId.replace(/[^a-zA-Z0-9_-]/g, "-")}`,
    [cardId],
  );
  const semanticMode = causal.semanticMode ?? inferSemanticModeFromNodes(causal.leftNodes);
  const isVerticalLayout = layoutOrientation === "vertical";

  const [paths, setPaths] = useState<GraphArrowPath[]>([]);
  const [previewImage, setPreviewImage] = useState<{ src: string; alt: string } | null>(null);
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const [expandedRightNodeId, setExpandedRightNodeId] = useState<string | null>(null);

  const containerRef = useRef<HTMLDivElement | null>(null);
  const leftNodeRefs = useRef<Map<string, HTMLDivElement>>(new Map());
  const rightNodeRefs = useRef<Map<string, HTMLDivElement>>(new Map());
  const recomputeRafRef = useRef<number | null>(null);
  const disposedRef = useRef(false);

  useEffect(() => {
    setPreviewImage(null);
    setSelectedNodeId(null);
    setExpandedRightNodeId(null);
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

  const rankedRightNodes = useMemo(() => {
    return [...causal.rightNodes].sort((a, b) => {
      const leftRank = a.rank ?? Number.MAX_SAFE_INTEGER;
      const rightRank = b.rank ?? Number.MAX_SAFE_INTEGER;
      if (leftRank !== rightRank) {
        return leftRank - rightRank;
      }
      return a.id.localeCompare(b.id);
    });
  }, [causal.rightNodes]);

  const useTailCompaction = cardType === "delivery_session";
  const hasOverflow = rankedRightNodes.length > TOP_VISIBLE_RIGHT_NODES;

  const primaryRightNodes = useMemo(() => {
    if (!hasOverflow) {
      return rankedRightNodes;
    }
    if (useTailCompaction) {
      return rankedRightNodes.slice(rankedRightNodes.length - TOP_VISIBLE_RIGHT_NODES);
    }
    return rankedRightNodes.slice(0, TOP_VISIBLE_RIGHT_NODES);
  }, [hasOverflow, rankedRightNodes, useTailCompaction]);

  const hiddenRightNodes = useMemo(() => {
    if (!hasOverflow) {
      return [];
    }
    if (useTailCompaction) {
      return rankedRightNodes.slice(0, rankedRightNodes.length - TOP_VISIBLE_RIGHT_NODES);
    }
    return rankedRightNodes.slice(TOP_VISIBLE_RIGHT_NODES);
  }, [hasOverflow, rankedRightNodes, useTailCompaction]);

  const overflowNode = useMemo<CausalNode | null>(() => {
    if (hiddenRightNodes.length === 0) {
      return null;
    }

    const lines = hiddenRightNodes.map((node) => {
      const title = resolveNodeTitle(node);
      const summary = (node.summaryLine ?? "").trim();
      const normalizedTitle = normalizeSemantic(title);
      const normalizedSummary = normalizeSemantic(summary);
      return (
        summary.length > 0 && normalizedSummary !== normalizedTitle
          ? `${title} — ${summary}`
          : title
      );
    });

    return {
      id: `${cardId}:right:overflow-summary`,
      text: lines.join("\n"),
      headline: overflowTitle(semanticMode),
      bullets: lines,
      details: lines.join("\n"),
      role: rankedRightNodes[0]?.role,
      rank: hiddenRightNodes[0]?.rank ?? 1,
      groupType: "overflow_summary",
      isImageApplicable: false,
    };
  }, [cardId, hiddenRightNodes, rankedRightNodes, semanticMode]);

  const visibleRightNodes = useMemo(() => {
    if (!overflowNode) {
      return primaryRightNodes;
    }
    return [
      ...primaryRightNodes,
      overflowNode,
    ];
  }, [overflowNode, primaryRightNodes]);

  const rightNodeStaggerOffsets = useMemo(() => {
    if (!isVerticalLayout) {
      return visibleRightNodes.map(() => 0);
    }
    const pattern = [-8, 7, -4, 10, -2, 6];
    return visibleRightNodes.map((node, index) => {
      if (node.groupType === "overflow_summary") {
        return 5;
      }
      return pattern[index % pattern.length];
    });
  }, [isVerticalLayout, visibleRightNodes]);

  const leftNodeIds = useMemo(
    () => new Set(causal.leftNodes.map((node) => node.id)),
    [causal.leftNodes],
  );
  const rightNodeIds = useMemo(
    () => new Set(rankedRightNodes.map((node) => node.id)),
    [rankedRightNodes],
  );
  const visibleRightNodeIds = useMemo(
    () => new Set(visibleRightNodes.map((node) => node.id)),
    [visibleRightNodes],
  );

  useEffect(() => {
    if (!selectedNodeId) {
      return;
    }
    if (leftNodeIds.has(selectedNodeId) || rightNodeIds.has(selectedNodeId)) {
      return;
    }
    setSelectedNodeId(null);
  }, [leftNodeIds, rightNodeIds, selectedNodeId]);

  const defaultExpandedRightNodeId = useMemo(() => {
    const firstPrimary = primaryRightNodes.find((node) => node.groupType !== "overflow_summary");
    return firstPrimary?.id ?? primaryRightNodes[0]?.id ?? null;
  }, [primaryRightNodes]);

  useEffect(() => {
    setExpandedRightNodeId(defaultExpandedRightNodeId);
  }, [cardId, defaultExpandedRightNodeId]);

  useEffect(() => {
    if (!expandedRightNodeId) {
      return;
    }
    if (visibleRightNodeIds.has(expandedRightNodeId)) {
      return;
    }
    setExpandedRightNodeId(defaultExpandedRightNodeId);
  }, [defaultExpandedRightNodeId, expandedRightNodeId, visibleRightNodeIds]);

  const visibleLinks = useMemo(() => {
    const base = causal.links.filter((link) => visibleRightNodeIds.has(link.toId));
    if (!overflowNode || causal.leftNodes.length === 0) {
      return base;
    }

    const overflowFromId = isVerticalLayout
      ? primaryRightNodes[Math.floor((primaryRightNodes.length - 1) / 2)]?.id ??
        causal.leftNodes[0].id
      : causal.leftNodes[0].id;

    return [
      ...base,
      {
        id: `${cardId}:link:overflow`,
        fromId: overflowFromId,
        toId: overflowNode.id,
        strength: 0.84,
      },
    ];
  }, [
    cardId,
    causal.leftNodes,
    causal.links,
    isVerticalLayout,
    overflowNode,
    primaryRightNodes,
    visibleRightNodeIds,
  ]);

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
    type LinkGeometry = {
      linkId: string;
      fromId: string;
      startX: number;
      startY: number;
      endX: number;
      endY: number;
    };

    const linkGeometries: LinkGeometry[] = [];
    for (const link of visibleLinks) {
      const fromElement =
        leftNodeRefs.current.get(link.fromId) ?? rightNodeRefs.current.get(link.fromId);
      const toElement =
        rightNodeRefs.current.get(link.toId) ?? leftNodeRefs.current.get(link.toId);

      if (!fromElement || !toElement) {
        continue;
      }

      const fromBounds = fromElement.getBoundingClientRect();
      const toBounds = toElement.getBoundingClientRect();
      const startX = isVerticalLayout
        ? fromBounds.left - containerBounds.left + fromBounds.width / 2
        : fromBounds.right - containerBounds.left + 0.5;
      const startY = isVerticalLayout
        ? fromBounds.bottom - containerBounds.top - 0.5
        : fromBounds.top - containerBounds.top + fromBounds.height / 2;
      const endX = isVerticalLayout
        ? toBounds.left - containerBounds.left + toBounds.width / 2
        : toBounds.left - containerBounds.left - 0.5;
      const endY = isVerticalLayout
        ? toBounds.top - containerBounds.top + 0.5
        : toBounds.top - containerBounds.top + toBounds.height / 2;

      linkGeometries.push({
        linkId: link.id ?? `${link.fromId}->${link.toId}`,
        fromId: link.fromId,
        startX,
        startY,
        endX,
        endY,
      });
    }

    const nextPaths: GraphArrowPath[] = [];
    if (isVerticalLayout) {
      const grouped = new Map<string, LinkGeometry[]>();
      for (const geometry of linkGeometries) {
        const bucket = grouped.get(geometry.fromId);
        if (bucket) {
          bucket.push(geometry);
        } else {
          grouped.set(geometry.fromId, [geometry]);
        }
      }

      for (const group of grouped.values()) {
        if (group.length === 0) continue;

        group.sort((a, b) => a.endX - b.endX);
        const sourceX = group[0].startX;
        const sourceY = group[0].startY;

        if (group.length > 1) {
          const minEndY = Math.min(...group.map((geometry) => geometry.endY));
          const maxHubY = minEndY - 14;
          const estimatedHub =
            sourceY + Math.max(24, Math.min(66, (minEndY - sourceY) * 0.34));
          const hubY = Math.max(sourceY + 12, Math.min(maxHubY, estimatedHub));

          if (hubY > sourceY + 8) {
            const trunkC1Y = sourceY + Math.max(10, (hubY - sourceY) * 0.36);
            const trunkC2Y = sourceY + Math.max(16, (hubY - sourceY) * 0.74);
            nextPaths.push({
              id: `${group[0].fromId}:trunk:${Math.round(hubY)}`,
              d: `M ${sourceX} ${sourceY} C ${sourceX} ${trunkC1Y}, ${sourceX} ${trunkC2Y}, ${sourceX} ${hubY}`,
              headD: "",
              isTrunk: true,
            });
          }

          const total = group.length;
          group.forEach((geometry, position) => {
            const laneOffset = (position - (total - 1) / 2) * 14;
            const laneX = sourceX + laneOffset;
            const endX = geometry.endX;
            const endY = geometry.endY;
            const c1x = sourceX + laneOffset * 0.58;
            const c1y = hubY + Math.max(9, Math.min(24, (endY - hubY) * 0.22));
            const c2x = endX;
            const c2y = endY - Math.max(12, Math.min(40, (endY - hubY) * 0.44));
            const pathD = `M ${sourceX} ${hubY} C ${c1x} ${c1y}, ${laneX} ${Math.max(
              hubY + 8,
              c2y - 12,
            )}, ${endX} ${endY}`;
            const headD = buildArrowHeadPath(endX, endY, c2x, c2y);
            nextPaths.push({
              id: geometry.linkId,
              d: pathD,
              headD,
            });
          });
          continue;
        }

        const only = group[0];
        const verticalDistance = Math.max(42, only.endY - only.startY);
        const c1x = only.startX;
        const c1y = only.startY + Math.min(34, verticalDistance * 0.34);
        const c2x = only.endX;
        const c2y = only.endY - Math.min(34, verticalDistance * 0.46);
        const pathD = `M ${only.startX} ${only.startY} C ${c1x} ${c1y}, ${c2x} ${c2y}, ${only.endX} ${only.endY}`;
        const headD = buildArrowHeadPath(only.endX, only.endY, c2x, c2y);
        nextPaths.push({
          id: only.linkId,
          d: pathD,
          headD,
        });
      }
    } else {
      const sourceCounter = new Map<string, number>();
      const sourceTotals = new Map<string, number>();
      for (const geometry of linkGeometries) {
        sourceTotals.set(
          geometry.fromId,
          (sourceTotals.get(geometry.fromId) ?? 0) + 1,
        );
      }

      for (const geometry of linkGeometries) {
        const position = sourceCounter.get(geometry.fromId) ?? 0;
        sourceCounter.set(geometry.fromId, position + 1);
        const total = sourceTotals.get(geometry.fromId) ?? 1;
        const spread = (position - (total - 1) / 2) * 7;
        const horizontalDistance = Math.max(42, geometry.endX - geometry.startX);
        const c1x = geometry.startX + horizontalDistance * 0.34;
        const c1y = geometry.startY + spread;
        const c2x = geometry.endX - horizontalDistance * 0.44;
        const c2y = geometry.endY;
        const pathD = `M ${geometry.startX} ${geometry.startY} C ${c1x} ${c1y}, ${c2x} ${c2y}, ${geometry.endX} ${geometry.endY}`;
        const headD = buildArrowHeadPath(geometry.endX, geometry.endY, c2x, c2y);
        nextPaths.push({
          id: geometry.linkId,
          d: pathD,
          headD,
        });
      }
    }

    setPaths((previous) => (pathsEqual(previous, nextPaths) ? previous : nextPaths));
  }, [isVerticalLayout, visibleLinks]);

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
  }, [scheduleLinkPathRecompute, visibleLinks, visibleRightNodes, causal.leftNodes]);

  useEffect(() => {
    if (IS_JSDOM_ENV) {
      return;
    }

    const handleResize = () => scheduleLinkPathRecompute();
    const scrollParent = containerRef.current?.closest(".life-stream-messages") ?? window;
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
    if (IS_JSDOM_ENV || typeof ResizeObserver === "undefined") {
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

  const selectedNode = useMemo(() => {
    if (!selectedNodeId) {
      return undefined;
    }
    return [...causal.leftNodes, ...rankedRightNodes].find(
      (node) => node.id === selectedNodeId,
    );
  }, [causal.leftNodes, rankedRightNodes, selectedNodeId]);

  const selectedNodeSide: "left" | "right" | null = useMemo(() => {
    if (!selectedNodeId) {
      return null;
    }
    if (leftNodeIds.has(selectedNodeId)) {
      return "left";
    }
    if (rightNodeIds.has(selectedNodeId)) {
      return "right";
    }
    return null;
  }, [leftNodeIds, rightNodeIds, selectedNodeId]);

  const splitSourceNodeIds = useMemo(() => {
    if (selectedNodeSide === "left" && selectedNodeId) {
      return [selectedNodeId];
    }
    if (causal.leftNodes[0]) {
      return [causal.leftNodes[0].id];
    }
    return [];
  }, [causal.leftNodes, selectedNodeId, selectedNodeSide]);

  const mergeSourceNodeIds = useMemo(() => {
    if (rankedRightNodes.length < 2) {
      return [];
    }
    if (!(selectedNodeSide === "right" && selectedNodeId)) {
      return rankedRightNodes.slice(0, 2).map((node) => node.id);
    }

    const selectedIndex = rankedRightNodes.findIndex((node) => node.id === selectedNodeId);
    if (selectedIndex < 0) {
      return rankedRightNodes.slice(0, 2).map((node) => node.id);
    }

    const selected = rankedRightNodes[selectedIndex];
    const partner =
      rankedRightNodes[selectedIndex + 1] ?? rankedRightNodes[selectedIndex - 1];
    if (!partner) {
      return [selected.id];
    }
    return [selected.id, partner.id];
  }, [rankedRightNodes, selectedNodeId, selectedNodeSide]);

  const handleSelectNode = useCallback(
    (nodeId: string) => {
      setSelectedNodeId((prev) => (prev === nodeId ? null : nodeId));
      if (visibleRightNodeIds.has(nodeId)) {
        setExpandedRightNodeId((prev) => (prev === nodeId ? null : nodeId));
      }
    },
    [visibleRightNodeIds],
  );

  const handleSplitCause = useCallback(() => {
    if (!splitSourceNodeIds.length) {
      return;
    }
    onRestructure("split_cause", { sourceNodeIds: splitSourceNodeIds });
  }, [onRestructure, splitSourceNodeIds]);

  const handleMergeEffects = useCallback(() => {
    if (mergeSourceNodeIds.length < 2) {
      return;
    }
    onRestructure("merge_effects", { sourceNodeIds: mergeSourceNodeIds });
  }, [mergeSourceNodeIds, onRestructure]);

  const handleRelink = useCallback(() => {
    onRestructure("relink_arrows", {
      sourceNodeIds: selectedNodeId ? [selectedNodeId] : undefined,
    });
  }, [onRestructure, selectedNodeId]);

  const handleReframe = useCallback(() => {
    const hasRewardRoles = rankedRightNodes.some((node) => node.role === "reward");
    onRestructure("reframe_mode", {
      sourceNodeIds: selectedNodeId ? [selectedNodeId] : undefined,
      targetMode: hasRewardRoles ? "cause_effect" : "action_reward",
    });
  }, [onRestructure, rankedRightNodes, selectedNodeId]);

  const canSplitCause = splitSourceNodeIds.length > 0;
  const canMergeEffects = rankedRightNodes.length >= 2;
  const selectedNodeTitle = useMemo(() => {
    if (!selectedNode) {
      return "";
    }

    const candidate =
      selectedNode.headline?.trim() ||
      selectedNode.title?.trim() ||
      selectedNode.bullets?.find((item) => item.trim().length > 0)?.trim() ||
      selectedNode.text.trim();

    if (!candidate) {
      return "";
    }
    return candidate.length > 84 ? `${candidate.slice(0, 84)}…` : candidate;
  }, [selectedNode]);

  const leftLaneLabel = laneLabel("left", semanticMode);
  const rightLaneLabel = laneLabel("right", semanticMode);

  return (
    <section
      className={`life-causal-card life-causal-card--${layoutOrientation}`}
      data-card-id={cardId}
    >
      <div className="life-causal-card__labels">
        <div className="life-causal-card__lane-label life-causal-card__lane-label--left">
          {leftLaneLabel}
        </div>
        <div className="life-causal-card__lane-label life-causal-card__lane-label--right">
          {rightLaneLabel}
        </div>
      </div>

      <div
        ref={containerRef}
        className={`life-causal-card__lanes life-causal-card__lanes--${layoutOrientation}`}
      >
        <div className="life-causal-card__lane life-causal-card__lane--left">
          {causal.leftNodes.map((node, index) => (
            <NodeCard
              key={node.id}
              node={node}
              side="left"
              index={index}
              layoutOrientation={layoutOrientation}
              staggerOffsetPx={0}
              fallbackTitle={index === 0 ? cardTitle : undefined}
              selected={selectedNodeId === node.id}
              setRef={registerLeftRef}
              onSelectNode={handleSelectNode}
              onOpenTranscript={onOpenTranscript}
              onRequestNodeImage={onRequestNodeImage}
              onPreviewImage={(src, alt) => setPreviewImage({ src, alt })}
            />
          ))}
        </div>

        <GraphArrowLayer
          paths={paths}
          gradientId={gradientId}
          orientation={layoutOrientation}
        />

        <div className="life-causal-card__lane life-causal-card__lane--right">
          {visibleRightNodes.map((node, index) => (
            <NodeCard
              key={node.id}
              node={node}
              side="right"
              index={index}
              layoutOrientation={layoutOrientation}
              staggerOffsetPx={rightNodeStaggerOffsets[index] ?? 0}
              selected={expandedRightNodeId === node.id}
              setRef={registerRightRef}
              onSelectNode={handleSelectNode}
              onRequestNodeImage={onRequestNodeImage}
              onPreviewImage={(src, alt) => setPreviewImage({ src, alt })}
            />
          ))}
        </div>
      </div>

      {selectedNode && (
        <div className="life-causal-card__context-toolbar" data-no-toggle>
          <span className="life-causal-card__context-selected">
            Selected {selectedNodeSide === "left" ? "source" : "response"}: {selectedNodeTitle}
          </span>
          <div className="life-causal-card__actions">
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
        </div>
      )}

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
