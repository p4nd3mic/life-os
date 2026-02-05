export type DownloadEvent = {
  event: string;
  data?: unknown;
};

export type Update = {
  available: boolean;
};

export async function check(): Promise<Update | null> {
  return { available: false };
}
