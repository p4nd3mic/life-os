export function normalizeStructuredMarkdown(input: string): string {
  return input
    .replace(/\r\n/g, "\n")
    .replace(/[ \t]+\n/g, "\n")
    .replace(/\n{3,}/g, "\n\n")
    .replace(/(^\s*[-*+]\s[^\n]+)\n{2,}(?=\s*[-*+]\s)/gm, "$1\n")
    .replace(/(^\s*\d+[.)]\s[^\n]+)\n{2,}(?=\s*\d+[.)]\s)/gm, "$1\n")
    .trim();
}

