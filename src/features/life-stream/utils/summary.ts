import { normalizeStructuredMarkdown } from "./markdownNormalize";

const HEADING_RE = /^(#{1,3})\s+(.+)$/;
const BOLD_HEADING_RE = /^\*\*(.+)\*\*$/;
const EMOJI_HEADING_RE = /^[\p{Emoji_Presentation}\p{Extended_Pictographic}]\s+(.+)$/u;
const NUMBERED_HEADER_RE = /^(\d+[.)])\s+(.+)$/;
const EMOJI_NUMBERED_HEADER_RE =
  /^[\p{Emoji_Presentation}\p{Extended_Pictographic}]\s*(\d+[.)])\s+(.+)$/u;
const LEADING_BULLET_MARKERS_RE = /^(?:[-*+]\s+|[•◦▪▫‣⁃·]\s+)+/;
const BULLET_RE =
  /^(?:[\p{Emoji_Presentation}\p{Extended_Pictographic}]\s*)?(?:\*|-|[•◦▪▫‣⁃·]|\d+[.)])\s+(.+)$/u;

type Section = {
  title: string;
  bullets: string[];
};

function cleanText(value: string): string {
  return value.replace(/\s+/g, " ").trim();
}

function stripLeadingDecorations(value: string): string {
  return value
    .replace(
      /^(?:[\p{Emoji_Presentation}\p{Extended_Pictographic}]\s*)*(?:[#*+\-]|\d+[.)])?\s*/u,
      "",
    )
    .trim();
}

function clampLine(value: string, maxLen = 140): string {
  if (value.length <= maxLen) return value;
  const trimmed = value.slice(0, maxLen);
  const lastSpace = trimmed.lastIndexOf(" ");
  if (lastSpace > 0) {
    return trimmed.slice(0, lastSpace).trim();
  }
  return trimmed.trim();
}

function isLikelyNumberedHeader(text: string): boolean {
  const words = text.split(/\s+/).filter(Boolean);
  return text.length >= 8 || words.length >= 2;
}

function normalizeForHeaderDetection(line: string): string {
  return line
    .replace(LEADING_BULLET_MARKERS_RE, "")
    .replace(/\*\*/g, "")
    .replace(/__/g, "")
    .replace(/`/g, "")
    .trim();
}

function isSectionHeader(line: string, isTopLevel: boolean): string | null {
  if (isTopLevel) {
    const normalizedLine = normalizeForHeaderDetection(line);
    const emojiNumbered = EMOJI_NUMBERED_HEADER_RE.exec(normalizedLine);
    if (emojiNumbered) {
      const numberedText = cleanText(emojiNumbered[2]);
      if (isLikelyNumberedHeader(numberedText)) {
        return `${emojiNumbered[1]} ${numberedText}`;
      }
    }
    const numbered = NUMBERED_HEADER_RE.exec(normalizedLine);
    if (numbered) {
      const numberedText = cleanText(numbered[2]);
      if (isLikelyNumberedHeader(numberedText)) {
        return `${numbered[1]} ${numberedText}`;
      }
    }
  }

  const headingMatch = HEADING_RE.exec(line);
  if (headingMatch) return cleanText(headingMatch[2]);
  const boldMatch = BOLD_HEADING_RE.exec(line);
  if (boldMatch) return cleanText(boldMatch[1]);
  const emojiMatch = EMOJI_HEADING_RE.exec(line);
  if (emojiMatch) {
    const rest = cleanText(emojiMatch[1]);
    if (/^(\d+[.)]|[-*])\s+/.test(rest)) return null;
    return rest;
  }
  return null;
}

function extractSections(markdown: string, maxSections = 4, maxBullets = 4): Section[] {
  const lines = markdown.split(/\r?\n/);
  const sections: Section[] = [];
  let current: Section | null = null;

  const pushSection = () => {
    if (!current) return;
    if (!current.title && current.bullets.length === 0) return;
    sections.push(current);
  };

  for (let i = 0; i < lines.length; i += 1) {
    const rawLine = lines[i];
    const raw = rawLine.trim();
    if (!raw) continue;
    const isTopLevel = !/^\s/.test(rawLine);
    const header = isSectionHeader(raw, isTopLevel);
    if (header) {
      if (sections.length >= maxSections) break;
      if (current) pushSection();
      current = { title: header, bullets: [] };
      continue;
    }
    const bulletMatch = BULLET_RE.exec(raw);
    if (bulletMatch) {
      if (!current) {
        current = { title: "", bullets: [] };
      }
      if (current.bullets.length < maxBullets) {
        current.bullets.push(stripLeadingDecorations(bulletMatch[1]));
      }
      continue;
    }

    if (current && current.bullets.length < maxBullets) {
      const sentence = raw.split(/(?<=[.!?])\s/)[0] ?? raw;
      if (sentence) {
        current.bullets.push(stripLeadingDecorations(sentence));
      }
    }
  }

  if (current) pushSection();
  return sections.slice(0, maxSections);
}

export function buildCollapsedSummary(markdown: string): string {
  const trimmed = normalizeStructuredMarkdown(markdown);
  if (!trimmed) return "";
  const sections = extractSections(trimmed);
  if (sections.length === 0) {
    const firstLines = trimmed.split(/\r?\n/).filter(Boolean).slice(0, 2);
    return firstLines
      .map((line) => `- ${clampLine(cleanText(stripLeadingDecorations(line)))}`)
      .join("\n");
  }

  const lines: string[] = [];
  sections.forEach((section) => {
    if (section.title) {
      lines.push(`### ${clampLine(section.title)}`);
    }
    const bullets =
      section.bullets.length > 0
        ? section.bullets
        : section.title
          ? []
          : [trimmed.split(/\r?\n/)[0] ?? ""];
    bullets.slice(0, 4).forEach((bullet) => {
      if (bullet) {
        lines.push(`- ${clampLine(cleanText(bullet))}`);
      }
    });
    lines.push("");
  });

  return lines.join("\n").trim();
}
