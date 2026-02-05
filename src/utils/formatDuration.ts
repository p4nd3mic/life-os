/**
 * Format a duration in milliseconds as a "M:SS" string.
 * @param ms Duration in milliseconds
 * @returns Formatted string like "1:05" or "0:30"
 */
export function formatDurationMs(ms: number): string {
  const totalSeconds = Math.max(0, Math.floor(ms / 1000));
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${minutes}:${seconds.toString().padStart(2, "0")}`;
}
