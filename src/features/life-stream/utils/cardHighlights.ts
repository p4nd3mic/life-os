import type { CardStatValue, CardType, DomainId, StreamCard } from "../types";

export type CardHighlight = {
  label: string;
  value: string;
};

type HighlightKey = string | string[];
type HighlightConfig = Array<{ key: HighlightKey; label: string }>;

const CARD_TYPE_HIGHLIGHT_CONFIG: Partial<Record<CardType, HighlightConfig>> = {
  delivery_session: [
    {
      key: ["earnings", "totalEarnings", "total_earnings", "total", "actual"],
      label: "$",
    },
    {
      key: ["orders", "orders_completed", "orderCount", "order_count", "orders_count"],
      label: "Orders",
    },
    {
      key: ["real_hours", "hours", "active_hours", "activeHours", "shift_hours"],
      label: "Hrs",
    },
    {
      key: ["hourly_real", "hourly_rate", "hourlyRate", "per_hour", "hourly"],
      label: "$/hr",
    },
    {
      key: ["real_miles", "miles", "totalMiles", "total_miles"],
      label: "Mi",
    },
    {
      key: ["per_mile_real", "per_mile_rate", "perMileRate", "per_mile", "dpm"],
      label: "$/mi",
    },
  ],
};

const HIGHLIGHT_CONFIG: Record<DomainId, HighlightConfig> = {
  nutrition: [
    { key: "calories", label: "Cal" },
    { key: "protein", label: "Protein" },
    { key: "carbs", label: "Carbs" },
    { key: "fat", label: "Fat" },
  ],
  delivery: [
    { key: "verdict", label: "Verdict" },
    { key: "pay", label: "$" },
    { key: "miles", label: "Mi" },
    { key: "dpm", label: "$/mi" },
  ],
  media: [
    { key: "type", label: "Type" },
    { key: "status", label: "Status" },
    { key: "year", label: "Year" },
    { key: "rating", label: "Rating" },
  ],
  youtube: [
    { key: "tier", label: "Tier" },
    { key: "status", label: "Status" },
  ],
  finance: [
    { key: "amount", label: "$" },
    { key: "category", label: "Category" },
    { key: "payee", label: "Payee" },
  ],
  fitness: [
    { key: "type", label: "Type" },
    { key: "duration", label: "Mins" },
  ],
  general: [],
};

function resolveStatValue(
  stats: Record<string, CardStatValue>,
  key: HighlightKey,
): { key: string; value: CardStatValue } | null {
  if (Array.isArray(key)) {
    for (const candidate of key) {
      if (candidate in stats) {
        return { key: candidate, value: stats[candidate] };
      }
    }
    return null;
  }
  if (key in stats) {
    return { key, value: stats[key] };
  }
  return null;
}

function formatStatValue(key: string, value: CardStatValue): string {
  if (value === null || value === undefined) return "";
  if (typeof value === "boolean") return value ? "Yes" : "No";
  if (typeof value === "number") {
    switch (key) {
      case "earnings":
      case "totalEarnings":
      case "total_earnings":
      case "total":
      case "actual":
        return `$${value.toFixed(2)}`;
      case "calories":
        return `${Math.round(value)} cal`;
      case "protein":
      case "carbs":
      case "fat":
      case "fiber":
        return `${Math.round(value)}g`;
      case "pay":
        return `$${value.toFixed(2)}`;
      case "orders":
      case "orderCount":
      case "order_count":
      case "orders_count":
      case "orders_completed":
        return `${Math.round(value)}`;
      case "miles":
      case "totalMiles":
      case "total_miles":
      case "real_miles":
      case "delivery_miles":
      case "deadhead_miles":
        return `${value.toFixed(1)} mi`;
      case "dpm":
      case "per_mile_rate":
      case "perMileRate":
      case "per_mile":
      case "per_mile_real":
      case "per_mile_delivery":
        return `$${value.toFixed(2)}/mi`;
      case "hourly_rate":
      case "hourlyRate":
      case "per_hour":
      case "hourly":
      case "hourly_real":
      case "hourly_active":
        return `$${value.toFixed(2)}/hr`;
      case "hours":
      case "active_hours":
      case "activeHours":
      case "shift_hours":
      case "real_hours":
        return `${value.toFixed(1)} hr`;
      case "duration":
        return `${Math.round(value)} min`;
      case "rating":
        return `${value}/10`;
      case "confidence":
        return value <= 1 ? `${Math.round(value * 100)}%` : `${Math.round(value)}%`;
      default:
        return value.toString();
    }
  }
  return String(value);
}

export function getCardHighlights(card: StreamCard): CardHighlight[] {
  const stats = card.stats ?? {};
  const config =
    CARD_TYPE_HIGHLIGHT_CONFIG[card.cardType] ??
    HIGHLIGHT_CONFIG[card.domain] ??
    [];
  const highlights: CardHighlight[] = [];

  for (const { key, label } of config) {
    const resolved = resolveStatValue(stats, key);
    if (!resolved) continue;
    const formatted = formatStatValue(resolved.key, resolved.value);
    if (!formatted) continue;
    highlights.push({ label, value: formatted });
  }

  return highlights;
}
