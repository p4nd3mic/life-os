import { useMemo } from "react";
import type { CausalNode } from "../../types";
import {
  BoardCard,
  type BoardCardColorVariant,
  type BoardCardResolvedNode,
} from "./BoardCard";
import "./CardBoardLayout.css";

export type BoardResolvedNodeContent = BoardCardResolvedNode;

type CardBoardLayoutProps = {
  cardId: string;
  rightNodes: CausalNode[];
  resolvedNodeContent: Map<string, BoardResolvedNodeContent>;
  selectedNodeId: string | null;
  setNodeRef: (nodeId: string, element: HTMLDivElement | null) => void;
  onSelectNode: (nodeId: string | null, options?: { toggle?: boolean }) => void;
  onOpenContextMenu: (nodeId: string, point: { x: number; y: number }) => void;
};

const COLOR_CYCLE: BoardCardColorVariant[] = ["gold", "green", "blue"];

function resolveColorVariant(index: number): BoardCardColorVariant {
  return COLOR_CYCLE[index % COLOR_CYCLE.length] ?? "gold";
}

function fallbackSummary(node: CausalNode): string {
  return node.summaryLine?.trim() || "";
}

export function CardBoardLayout({
  cardId,
  rightNodes,
  resolvedNodeContent,
  selectedNodeId,
  setNodeRef,
  onSelectNode,
  onOpenContextMenu,
}: CardBoardLayoutProps) {
  const rows = useMemo(() => {
    return rightNodes.map((node, index) => {
      const rank = node.rank ?? index + 1;
      const resolved = resolvedNodeContent.get(node.id) ?? {
        title: node.headline?.trim() || node.title?.trim() || node.text.trim(),
        summaryLine: fallbackSummary(node),
        bullets: node.bullets ?? [],
      };

      return {
        node,
        rank,
        resolved,
        colorVariant: resolveColorVariant(index),
      };
    });
  }, [resolvedNodeContent, rightNodes]);

  if (rows.length === 0) {
    return (
      <div className="card-board-layout card-board-layout--empty">
        <p className="card-board-layout__empty">No ranked cards available yet.</p>
      </div>
    );
  }

  return (
    <div className="card-board-layout" data-card-id={cardId}>
      <div className="card-board-layout__grid">
        {rows.map(({ node, rank, resolved, colorVariant }) => (
          <BoardCard
            key={node.id}
            node={node}
            rank={rank}
            colorVariant={colorVariant}
            resolved={resolved}
            isSelected={selectedNodeId === node.id}
            setRef={setNodeRef}
            onSelect={onSelectNode}
            onOpenContextMenu={onOpenContextMenu}
          />
        ))}
      </div>
    </div>
  );
}
