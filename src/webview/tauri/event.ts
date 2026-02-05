export type TauriEvent<T> = {
  event: string;
  id: number;
  payload: T;
};

export type EventCallback<T> = (event: TauriEvent<T>) => void;
export type UnlistenFn = () => void;

export async function listen<T>(
  eventName: string,
  handler: EventCallback<T>,
): Promise<UnlistenFn> {
  const listener = (event: Event) => {
    const payload = (event as CustomEvent).detail as T;
    handler({ event: eventName, id: Date.now(), payload });
  };
  window.addEventListener(eventName, listener as EventListener);
  return () => window.removeEventListener(eventName, listener as EventListener);
}

export async function emit(eventName: string, payload?: unknown): Promise<void> {
  window.dispatchEvent(new CustomEvent(eventName, { detail: payload }));
}
