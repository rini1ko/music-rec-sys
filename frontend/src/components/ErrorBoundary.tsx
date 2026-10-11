import { Component, type ErrorInfo, type ReactNode } from "react";
import { Button } from "./ui/Button";
import { EmptyState } from "./ui/EmptyState";

interface Props {
  /** A new value clears the error, e.g. the pathname after a navigation. */
  resetKey: string;
  children: ReactNode;
}

interface State {
  error: Error | null;
}

/** Shows a message instead of a blank page when a page crashes while rendering. */
export class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): Partial<State> {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("page crashed", error, info.componentStack);
  }

  componentDidUpdate(previous: Props) {
    if (this.state.error !== null && previous.resetKey !== this.props.resetKey) {
      this.setState({ error: null });
    }
  }

  render() {
    if (this.state.error === null) return this.props.children;
    return (
      <EmptyState
        className="mt-12"
        title="This page crashed"
        description={this.state.error.message}
        action={
          <Button variant="primary" onClick={() => this.setState({ error: null })}>
            Try again
          </Button>
        }
      />
    );
  }
}
