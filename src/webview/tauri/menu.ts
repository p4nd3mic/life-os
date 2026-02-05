type MenuItemOptions = {
  text: string;
  action?: () => void;
};

export class MenuItem {
  text: string;
  action?: () => void;

  protected constructor(text: string, action?: () => void) {
    this.text = text;
    this.action = action;
  }

  static async new(options: MenuItemOptions): Promise<MenuItem> {
    return new MenuItem(options.text, options.action);
  }
}

export class PredefinedMenuItem extends MenuItem {
  constructor(text: string, action?: () => void) {
    super(text, action);
  }

  static async new(options: MenuItemOptions): Promise<MenuItem> {
    return new PredefinedMenuItem(options.text, options.action);
  }
}

export class Menu {
  items: MenuItem[];

  private constructor(items: MenuItem[]) {
    this.items = items;
  }

  static async new(options: { items: MenuItem[] }): Promise<Menu> {
    return new Menu(options.items);
  }

  async popup(_position?: unknown, _window?: unknown): Promise<void> {
    // No-op in webview stub.
  }
}
