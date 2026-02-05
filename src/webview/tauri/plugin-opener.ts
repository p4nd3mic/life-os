export async function openUrl(url: string): Promise<void> {
  if (typeof window !== "undefined") {
    window.open(url, "_blank");
  }
}

export async function revealItemInDir(_path: string): Promise<void> {
  // Not supported in webview stub.
}
