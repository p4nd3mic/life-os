function sanitizeAnchor(value: string): string {
  return value.replace(/[^a-zA-Z0-9_-]/g, "-");
}

export function cardAnchorId(cardId: string): string {
  return `life-card-row-${sanitizeAnchor(cardId)}`;
}

export function nodeAnchorId(nodeId: string): string {
  return `life-causal-node-${sanitizeAnchor(nodeId)}`;
}

