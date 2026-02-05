import { initCodexBridge } from "../bridge";

type InvokeArgs = Record<string, unknown> | null | undefined;

export async function invoke<T = unknown>(
  method: string,
  args?: InvokeArgs,
): Promise<T> {
  const bridge = initCodexBridge();
  return (await bridge.invoke(method, args ?? null)) as T;
}

export function isTauri(): boolean {
  return false;
}

export function convertFileSrc(path: string): string {
  if (!path) return "";
  if (
    path.startsWith("http://") ||
    path.startsWith("https://") ||
    path.startsWith("data:") ||
    path.startsWith("codex://") ||
    path.startsWith("asset://")
  ) {
    return path;
  }
  const bridge = initCodexBridge();
  const params = new URLSearchParams();
  params.set("path", path);
  if (bridge.workspaceId) {
    params.set("workspaceId", bridge.workspaceId);
  }
  return `codex://file?${params.toString()}`;
}
