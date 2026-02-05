import type { ReactNode } from "react";
import { LifeStreamHeaderControls } from "./LifeStreamHeaderControls";
import "./LifeTopbar.css";

type LifeTopbarProps = {
  actionsNode?: ReactNode;
};

export function LifeTopbar({ actionsNode }: LifeTopbarProps) {
  return (
    <div className="main-topbar life-topbar" data-tauri-drag-region>
      <div className="life-topbar-left" data-tauri-drag-region="false">
        <span className="life-topbar-badge">
          <span className="life-topbar-badge-text">Life OS</span>
        </span>
      </div>
      <div className="life-topbar-center" data-tauri-drag-region="false">
        <LifeStreamHeaderControls />
      </div>
      <div className="life-topbar-right" data-tauri-drag-region="false">
        {actionsNode ?? null}
      </div>
    </div>
  );
}
