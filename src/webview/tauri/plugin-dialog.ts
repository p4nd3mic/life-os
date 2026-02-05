export async function open(_options?: unknown): Promise<string | string[] | null> {
  return null;
}

export async function ask(
  _message: string,
  _options?: { title?: string; type?: "info" | "warning" | "error" },
): Promise<boolean> {
  return false;
}

export async function message(
  message: string,
  _options?: { title?: string; type?: "info" | "warning" | "error" },
): Promise<void> {
  if (typeof window !== "undefined") {
    window.alert(message);
  }
}
