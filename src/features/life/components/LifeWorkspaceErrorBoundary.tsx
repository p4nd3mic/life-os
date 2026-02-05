import type { ReactNode } from "react";
import React, { Component } from "react";

type LifeWorkspaceErrorBoundaryProps = {
  children: ReactNode;
};

type LifeWorkspaceErrorBoundaryState = {
  hasError: boolean;
  error: Error | null;
  componentStack: string | null;
};

export class LifeWorkspaceErrorBoundary extends Component<
  LifeWorkspaceErrorBoundaryProps,
  LifeWorkspaceErrorBoundaryState
> {
  state: LifeWorkspaceErrorBoundaryState = {
    hasError: false,
    error: null,
    componentStack: null,
  };

  static getDerivedStateFromError(error: Error): LifeWorkspaceErrorBoundaryState {
    return { hasError: true, error, componentStack: null };
  }

  componentDidCatch(error: Error, errorInfo: React.ErrorInfo) {
    console.error("LifeWorkspaceView crashed:", error, errorInfo);
    this.setState({ componentStack: errorInfo.componentStack ?? null });
  }

  handleRetry = () => {
    this.setState({ hasError: false, error: null, componentStack: null });
  };

  render() {
    if (this.state.hasError) {
      return (
        <div className="life-error">
          <p>Something went wrong. Try refreshing.</p>
          {this.state.error && (
            <pre className="life-error__details">
              {this.state.error.message}
              {this.state.error.stack ? `\n${this.state.error.stack}` : ""}
            </pre>
          )}
          {this.state.componentStack && (
            <pre className="life-error__details">{this.state.componentStack}</pre>
          )}
          <button
            type="button"
            onClick={this.handleRetry}
            className="life-error__retry"
          >
            Retry
          </button>
        </div>
      );
    }

    return this.props.children;
  }
}
