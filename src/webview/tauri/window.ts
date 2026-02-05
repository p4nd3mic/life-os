export enum Effect {
  HudWindow = "HudWindow",
}

export enum EffectState {
  Active = "active",
  Inactive = "inactive",
  FollowsWindowActiveState = "follows",
}

type UnlistenFn = () => void;

const stubWindow = {
  label: "webview",
  async listen(_event: string, _handler: (...args: any[]) => void): Promise<UnlistenFn> {
    return () => {};
  },
  async onDragDropEvent(_handler: (...args: any[]) => void): Promise<UnlistenFn> {
    return () => {};
  },
  async startDragging(): Promise<void> {},
  async setEffects(_effects: any): Promise<void> {},
  async setTitle(_title: string): Promise<void> {},
  async innerSize(): Promise<{ width: number; height: number }> {
    return { width: window.innerWidth, height: window.innerHeight };
  },
  async innerPosition(): Promise<{ x: number; y: number }> {
    return { x: 0, y: 0 };
  },
  async show(): Promise<void> {},
  async hide(): Promise<void> {},
  async setAlwaysOnTop(_flag: boolean): Promise<void> {},
  async center(): Promise<void> {},
};

export function getCurrentWindow() {
  return stubWindow;
}
