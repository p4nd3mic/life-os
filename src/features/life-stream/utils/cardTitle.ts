import type { CardType, StreamCard } from "../types";

const CARD_TYPE_LABELS: Record<CardType, string> = {
  meal: "Meal Log",
  delivery_order: "Delivery Log",
  delivery_session: "Delivery Session",
  media_add: "Media Log",
  music: "Media Log",
  thought: "Thought",
  query: "Query",
  code_task: "Code Task",
  generic: "Response",
};

const DOMAIN_EMOJI: Record<string, string> = {
  general: "📝",
  nutrition: "🍽️",
  delivery: "🚗",
  media: "🎬",
  youtube: "🎥",
  finance: "💸",
  fitness: "🏋️",
};

const GENERIC_TITLES = new Set(["response", "thought", "query", "generic"]);

function isGenericTitle(value: string): boolean {
  return GENERIC_TITLES.has(value.trim().toLowerCase());
}

const FILLER_PREFIX_RE =
  /^(so|okay|ok|alright|all right|hey|well|hmm|uh|um|like|just|i think|i feel|i guess|i want to|i wanna)\b[\s,–-]*/i;

function isTruncatedTitle(title: string, input?: string): boolean {
  if (!input) return false;
  const trimmedTitle = title.trim();
  const trimmedInput = input.trim();
  if (!trimmedTitle || trimmedTitle.length >= trimmedInput.length) {
    return false;
  }
  const endsWithPunct = /[.!?…]$/.test(trimmedTitle);
  if (endsWithPunct) return false;
  return trimmedInput.toLowerCase().startsWith(trimmedTitle.toLowerCase());
}

function deriveTitleFromInput(input?: string): string | null {
  if (!input) return null;
  const cleaned = input
    .replace(/[\r\n]+/g, " ")
    .replace(/\s+/g, " ")
    .trim();
  if (!cleaned) return null;
  const withoutFiller = cleaned.replace(FILLER_PREFIX_RE, "");
  const candidate = withoutFiller.length > 0 ? withoutFiller : cleaned;
  const firstClause =
    candidate.split(/[.!?]/)[0]?.trim() ?? candidate;
  if (!firstClause) return null;
  const maxLen = 130;
  if (firstClause.length <= maxLen) {
    return firstClause;
  }
  const trimmed = firstClause.slice(0, maxLen);
  const lastSpace = trimmed.lastIndexOf(" ");
  if (lastSpace > 0) {
    return trimmed.slice(0, lastSpace).trim();
  }
  return trimmed.trim();
}

export function intentTitleFromType(cardType: CardType): string {
  return CARD_TYPE_LABELS[cardType] ?? "Response";
}

export function resolveCardTitle(card: StreamCard): string {
  const label = intentTitleFromType(card.cardType);
  const title = card.title?.trim();
  if (!title || isGenericTitle(title) || isTruncatedTitle(title, card.originalInput)) {
    return deriveTitleFromInput(card.originalInput) ?? label;
  }
  return title;
}

export function resolveCardTitleWithIcon(card: StreamCard): string {
  const baseTitle = resolveCardTitle(card);
  const emoji =
    card.emoji?.trim() ||
    DOMAIN_EMOJI[card.domain] ||
    DOMAIN_EMOJI.general;
  if (!emoji) {
    return baseTitle;
  }
  const trimmed = baseTitle.trim();
  if (trimmed.startsWith(emoji)) {
    return trimmed;
  }
  return `${emoji} ${trimmed}`;
}
