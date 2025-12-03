/**
 * Audio Routing - Channel routing matrix for audio capture
 */

export type RoutingConfig = number[][];

export class RoutingMatrix {
  private channelCount: number;
  private routing: RoutingConfig;
  private onRoutingChange: ((routing: RoutingConfig) => void) | null = null;

  constructor(channelCount: number) {
    this.channelCount = channelCount;
    // Initialize identity matrix (each input maps to itself)
    this.routing = Array.from({ length: channelCount }, (_, i) =>
      Array.from({ length: channelCount }, (_, j) => (i === j ? 1 : 0)),
    );
  }

  setOnRoutingChange(callback: (routing: RoutingConfig) => void): void {
    this.onRoutingChange = callback;
  }

  getRouting(): RoutingConfig {
    return this.routing;
  }

  setRouting(routing: RoutingConfig): void {
    this.routing = routing;
    if (this.onRoutingChange) {
      this.onRoutingChange(routing);
    }
  }

  getChannelCount(): number {
    return this.channelCount;
  }

  updateChannelCount(newCount: number): void {
    if (newCount === this.channelCount) return;

    this.channelCount = newCount;
    // Reinitialize routing matrix with new channel count
    this.routing = Array.from({ length: newCount }, (_, i) =>
      Array.from({ length: newCount }, (_, j) => (i === j ? 1 : 0)),
    );
    if (this.onRoutingChange) {
      this.onRoutingChange(this.routing);
    }
  }

  show(_targetElement: HTMLElement): void {
    // Placeholder for UI display logic
    console.log("Showing routing matrix UI (not implemented)");
    // In a full implementation, this would create/show a modal or popover
    // to display and edit the routing matrix
  }
}
