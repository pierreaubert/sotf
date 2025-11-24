// RoomEQ workflow manager - handles multi-channel room EQ optimization

import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import type {
  RoomEQConfig,
  RoomEQResult,
  RoomEQProgress,
  RoomEQWorkflowState,
  ChannelState,
  ChannelStatus,
  RoomConfigType,
  ChannelConfig,
} from "../types/roomeq";
import type { OptimizationParams } from "../types/optimization";
import { OPTIMIZATION_DEFAULTS } from "./optimization-constants";
import {
  createChannelState,
  getChannelCount,
  validateRoomEQConfig,
} from "../types/roomeq";

export class RoomEQManager {
  // Workflow state
  private workflowState: RoomEQWorkflowState;
  private progressUnlisten: UnlistenFn | null = null;

  // Event callbacks
  private onWorkflowStateChange?: (state: RoomEQWorkflowState) => void;
  private onChannelProgress?: (
    channelIndex: number,
    channelName: string,
    stage: string,
    progress: number,
  ) => void;
  private onOptimizationComplete?: (result: RoomEQResult) => void;
  private onError?: (error: string) => void;

  constructor() {
    this.workflowState = {
      step: 0,
      channels: [],
      is_running: false,
    };
    this.setupProgressListener();
  }

  /**
   * Set event callbacks
   */
  setCallbacks(callbacks: {
    onWorkflowStateChange?: (state: RoomEQWorkflowState) => void;
    onChannelProgress?: (
      channelIndex: number,
      channelName: string,
      stage: string,
      progress: number,
    ) => void;
    onOptimizationComplete?: (result: RoomEQResult) => void;
    onError?: (error: string) => void;
  }): void {
    this.onWorkflowStateChange = callbacks.onWorkflowStateChange;
    this.onChannelProgress = callbacks.onChannelProgress;
    this.onOptimizationComplete = callbacks.onOptimizationComplete;
    this.onError = callbacks.onError;
  }

  /**
   * Setup progress listener for room EQ optimization
   */
  private async setupProgressListener(): Promise<void> {
    try {
      this.progressUnlisten = await listen(
        "roomeq_progress",
        (event: { payload: RoomEQProgress }) => {
          this.handleProgressUpdate(event.payload);
        },
      );
    } catch (error) {
      console.error("[ROOMEQ] Failed to setup progress listener:", error);
    }
  }

  /**
   * Handle progress update from backend
   */
  private handleProgressUpdate(progress: RoomEQProgress): void {
    const { channel_index, channel_name, stage, progress: progressData } = progress;

    // Update channel state
    if (channel_index < this.workflowState.channels.length) {
      const channel = this.workflowState.channels[channel_index];
      channel.status = "optimizing";
      channel.stage = stage;

      // Calculate percentage from iteration (rough estimate)
      const estimatedMaxIterations = 300;
      const percentage = Math.min(
        100,
        (progressData.iteration / estimatedMaxIterations) * 100,
      );
      channel.progress = percentage;

      this.workflowState.current_channel_index = channel_index;
      this.notifyStateChange();

      // Notify progress callback
      if (this.onChannelProgress) {
        this.onChannelProgress(channel_index, channel_name, stage, percentage);
      }
    }
  }

  /**
   * Initialize workflow with a preset
   */
  initializeWorkflow(
    configType: RoomConfigType,
    channelNames: string[],
  ): void {
    this.workflowState = {
      step: 1, // Move to channel setup step
      config_type: configType,
      channels: channelNames.map((name) =>
        createChannelState({ channel_name: name }),
      ),
      is_running: false,
    };
    this.notifyStateChange();
  }

  /**
   * Set configuration type
   */
  setConfigType(configType: RoomConfigType): void {
    this.workflowState.config_type = configType;
    this.workflowState.step = 1;

    // Initialize channels based on config type
    const count = getChannelCount(configType);
    const channelNames = this.generateDefaultChannelNames(configType);

    this.workflowState.channels = channelNames.map((name) =>
      createChannelState({ channel_name: name }),
    );

    this.notifyStateChange();
  }

  /**
   * Generate default channel names based on config type
   */
  private generateDefaultChannelNames(configType: RoomConfigType): string[] {
    switch (configType.type) {
      case "single":
        return ["Single"];
      case "stereo_pair":
        return ["Left", "Right"];
      case "multi_channel":
        return Array.from(
          { length: configType.channel_count },
          (_, i) => `Channel ${i + 1}`,
        );
      case "multi_way":
        return ["Multi-Way Speaker"];
      default:
        return ["Channel 1"];
    }
  }

  /**
   * Update a specific channel configuration
   */
  updateChannel(index: number, config: Partial<ChannelConfig>): void {
    if (index < this.workflowState.channels.length) {
      this.workflowState.channels[index].config = {
        ...this.workflowState.channels[index].config,
        ...config,
      };
      this.notifyStateChange();
    }
  }

  /**
   * Add a new channel
   */
  addChannel(name?: string): void {
    const channelName = name || `Channel ${this.workflowState.channels.length + 1}`;
    this.workflowState.channels.push(
      createChannelState({ channel_name: channelName }),
    );
    this.notifyStateChange();
  }

  /**
   * Remove a channel
   */
  removeChannel(index: number): void {
    if (index < this.workflowState.channels.length) {
      this.workflowState.channels.splice(index, 1);
      this.notifyStateChange();
    }
  }

  /**
   * Set optimizer parameters
   */
  setOptimizerParams(params: Partial<OptimizationParams>): void {
    this.workflowState.optimizer_params = {
      ...(this.workflowState.optimizer_params || this.getDefaultOptimizerParams()),
      ...params,
    };
    this.notifyStateChange();
  }

  /**
   * Get default optimizer parameters
   */
  private getDefaultOptimizerParams(): OptimizationParams {
    return {
      num_filters: OPTIMIZATION_DEFAULTS.num_filters,
      sample_rate: OPTIMIZATION_DEFAULTS.sample_rate,
      max_db: OPTIMIZATION_DEFAULTS.max_db,
      min_db: OPTIMIZATION_DEFAULTS.min_db,
      max_q: OPTIMIZATION_DEFAULTS.max_q,
      min_q: OPTIMIZATION_DEFAULTS.min_q,
      min_freq: OPTIMIZATION_DEFAULTS.min_freq,
      max_freq: OPTIMIZATION_DEFAULTS.max_freq,
      curve_name: OPTIMIZATION_DEFAULTS.curve_name,
      algo: OPTIMIZATION_DEFAULTS.algo,
      population: OPTIMIZATION_DEFAULTS.population,
      maxeval: OPTIMIZATION_DEFAULTS.maxeval,
      refine: false,
      local_algo: OPTIMIZATION_DEFAULTS.local_algo,
      min_spacing_oct: OPTIMIZATION_DEFAULTS.min_spacing_oct,
      spacing_weight: OPTIMIZATION_DEFAULTS.spacing_weight,
      smooth: false,
      smooth_n: OPTIMIZATION_DEFAULTS.smooth_n,
      loss: OPTIMIZATION_DEFAULTS.loss,
      iir_hp_pk: false,
      tolerance: OPTIMIZATION_DEFAULTS.tolerance,
      atolerance: OPTIMIZATION_DEFAULTS.abs_tolerance,
    };
  }

  /**
   * Move to next workflow step
   */
  nextStep(): boolean {
    if (this.workflowState.step < 4) {
      this.workflowState.step++;
      this.notifyStateChange();
      return true;
    }
    return false;
  }

  /**
   * Move to previous workflow step
   */
  previousStep(): boolean {
    if (this.workflowState.step > 0) {
      this.workflowState.step--;
      this.notifyStateChange();
      return true;
    }
    return false;
  }

  /**
   * Go to a specific step
   */
  goToStep(step: number): void {
    if (step >= 0 && step <= 4) {
      this.workflowState.step = step;
      this.notifyStateChange();
    }
  }

  /**
   * Run room EQ optimization
   */
  async runOptimization(): Promise<RoomEQResult> {
    // Validate configuration
    const config = this.buildRoomEQConfig();
    const errors = validateRoomEQConfig(config);

    if (errors.length > 0) {
      const errorMessage = errors.join("; ");
      if (this.onError) {
        this.onError(errorMessage);
      }
      throw new Error(errorMessage);
    }

    // Mark as running
    this.workflowState.is_running = true;
    this.workflowState.channels.forEach((ch) => {
      ch.status = "pending";
      ch.progress = 0;
    });
    this.notifyStateChange();

    try {
      console.log("[ROOMEQ] Starting room EQ optimization");
      console.log("[ROOMEQ] Config:", JSON.stringify(config, null, 2));

      const result = (await invoke("run_roomeq_optimization", {
        config,
      })) as RoomEQResult;

      console.log("[ROOMEQ] Optimization complete");
      console.log("[ROOMEQ] Result:", result);

      // Update channel states with results
      for (let i = 0; i < result.channel_results.length; i++) {
        const channelResult = result.channel_results[i];
        if (i < this.workflowState.channels.length) {
          this.workflowState.channels[i].status = channelResult.success
            ? "complete"
            : "error";
          this.workflowState.channels[i].result = channelResult;
          this.workflowState.channels[i].progress = 100;
        }
      }

      this.workflowState.overall_result = result;
      this.workflowState.is_running = false;
      this.notifyStateChange();

      if (this.onOptimizationComplete) {
        this.onOptimizationComplete(result);
      }

      return result;
    } catch (error) {
      console.error("[ROOMEQ] Optimization failed:", error);
      this.workflowState.is_running = false;
      this.workflowState.channels.forEach((ch) => {
        if (ch.status === "pending" || ch.status === "optimizing") {
          ch.status = "error";
        }
      });
      this.notifyStateChange();

      const errorMessage =
        error instanceof Error ? error.message : "Unknown error";
      if (this.onError) {
        this.onError(errorMessage);
      }
      throw error;
    }
  }

  /**
   * Cancel optimization
   */
  async cancelOptimization(): Promise<void> {
    try {
      await invoke("cancel_roomeq_optimization");
      this.workflowState.is_running = false;
      this.notifyStateChange();
    } catch (error) {
      console.error("[ROOMEQ] Failed to cancel optimization:", error);
      throw error;
    }
  }

  /**
   * Build RoomEQConfig from current state
   */
  private buildRoomEQConfig(): RoomEQConfig {
    if (!this.workflowState.config_type) {
      throw new Error("Configuration type not set");
    }

    return {
      config_type: this.workflowState.config_type,
      channels: this.workflowState.channels.map((ch) => ch.config),
      optimizer_params:
        this.workflowState.optimizer_params || this.getDefaultOptimizerParams(),
    };
  }

  /**
   * Get current workflow state
   */
  getState(): RoomEQWorkflowState {
    return { ...this.workflowState };
  }

  /**
   * Get channel state
   */
  getChannel(index: number): ChannelState | null {
    return this.workflowState.channels[index] || null;
  }

  /**
   * Get all channel states
   */
  getChannels(): ChannelState[] {
    return [...this.workflowState.channels];
  }

  /**
   * Check if optimization is running
   */
  isRunning(): boolean {
    return this.workflowState.is_running;
  }

  /**
   * Get overall result
   */
  getResult(): RoomEQResult | null {
    return this.workflowState.overall_result || null;
  }

  /**
   * Reset workflow to initial state
   */
  reset(): void {
    this.workflowState = {
      step: 0,
      channels: [],
      is_running: false,
    };
    this.notifyStateChange();
  }

  /**
   * Notify state change to callbacks
   */
  private notifyStateChange(): void {
    if (this.onWorkflowStateChange) {
      this.onWorkflowStateChange(this.getState());
    }
  }

  /**
   * Cleanup
   */
  destroy(): void {
    if (this.progressUnlisten) {
      this.progressUnlisten();
      this.progressUnlisten = null;
    }

    if (this.workflowState.is_running) {
      this.cancelOptimization().catch(console.error);
    }
  }
}
