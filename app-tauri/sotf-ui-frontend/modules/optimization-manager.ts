// Optimization management and progress tracking

import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import type {
  OptimizationParams,
  OptimizationResult,
  ProgressData,
  OptimizationStage,
  PeqModel,
} from "../types/optimization";
import { OPTIMIZATION_DEFAULTS } from "./optimization-constants";

export class OptimizationManager {
  // Optimization state
  private isOptimizationRunning: boolean = false;
  private currentOptimizationId: string | null = null;
  private optimizationStages: OptimizationStage[] = [];
  private progressData: ProgressData[] = [];
  private progressUnlisten: UnlistenFn | null = null;

  // Captured data storage
  private capturedFrequencies: number[] | null = null;
  private capturedMagnitudes: number[] | null = null;

  // Optimization result storage
  private lastFilterParams: number[] | null = null;
  private lastSampleRate: number | null = null;
  private lastPeqModel: PeqModel | null = null;
  private lastLossType: string | null = null;
  private lastSpeakerName: string | null = null;

  // Event callbacks
  private onProgressUpdate?: (
    stage: string,
    status: string,
    details: string,
    percentage: number,
  ) => void;
  private onOptimizationComplete?: (result: OptimizationResult) => void;
  private onOptimizationError?: (error: string) => void;
  private onProgressDataUpdate?: (
    iteration: number,
    fitness: number,
    convergence: number,
  ) => void;

  constructor() {
    this.setupProgressListener();
  }

  setCallbacks(callbacks: {
    onProgressUpdate?: (
      stage: string,
      status: string,
      details: string,
      percentage: number,
    ) => void;
    onOptimizationComplete?: (result: OptimizationResult) => void;
    onOptimizationError?: (error: string) => void;
    onProgressDataUpdate?: (
      iteration: number,
      fitness: number,
      convergence: number,
    ) => void;
  }): void {
    this.onProgressUpdate = callbacks.onProgressUpdate;
    this.onOptimizationComplete = callbacks.onOptimizationComplete;
    this.onOptimizationError = callbacks.onOptimizationError;
    this.onProgressDataUpdate = callbacks.onProgressDataUpdate;
  }

  private async setupProgressListener(): Promise<void> {
    try {
      // Listen to optimization-progress events
      this.progressUnlisten = await listen("optimization-progress", (event) => {
        const data = event.payload as Record<string, unknown>;
        this.handleProgressEvent(data);
      });

      // Also listen to progress events (alternative name)
      await listen("progress", (event) => {
        const data = event.payload as Record<string, unknown>;
        this.handleProgressEvent(data);
      });

      // Listen to optimization_progress events (underscore variant)
      await listen("optimization_progress", (event) => {
        const data = event.payload as Record<string, unknown>;
        this.handleProgressEvent(data);
      });

      // Listen to iteration events (another possible name)
      await listen("iteration", (event) => {
        const data = event.payload as Record<string, unknown>;
        this.handleProgressEvent(data);
      });

      // Listen to progress_update events (the actual event name from Rust!)
      await listen("progress_update", (event) => {
        const data = event.payload as Record<string, unknown>;
        this.handleProgressEvent(data);
      });
    } catch (error) {
      console.error("❌ Failed to setup progress listener:", error);
    }
  }

  private handleProgressEvent(data: Record<string, unknown>): void {
    // Handle both old format (stage/status) and new format (direct progress data)
    if (data.stage && data.status) {
      this.handleProgressUpdate(data);
    } else if (data.iteration !== undefined || data.fitness !== undefined) {
      // Direct progress data format
      this.handleProgressUpdate(data);
    }
  }

  private handleProgressUpdate(data: Record<string, unknown>): void {
    const { stage, status, details = "", percentage } = data;

    // Calculate percentage if we have iteration data
    let calculatedPercentage = typeof percentage === "number" ? percentage : 0;

    // Update optimization stages
    this.updateOptimizationStage(
      typeof stage === "string" ? stage : "",
      typeof status === "string" ? status : "",
      typeof details === "string" ? String(details) : "",
    );

    // Store progress data if it contains fitness information
    if (data.iteration !== undefined && data.fitness !== undefined) {
      const iteration = typeof data.iteration === "number" ? data.iteration : 0;
      const fitness = typeof data.fitness === "number" ? data.fitness : 0;
      const convergence =
        typeof data.convergence === "number" ? data.convergence : 0;

      const progressEntry: ProgressData = {
        iteration,
        fitness,
        convergence,
      };

      this.progressData.push(progressEntry);

      // Calculate percentage based on iteration (assuming max 300 iterations as seen in Rust output)
      // This is a rough estimate - could be made more accurate with max_iter from backend
      const estimatedMaxIterations = 300;
      calculatedPercentage = Math.min(
        100,
        (iteration / estimatedMaxIterations) * 100,
      );
      // Notify progress data callback for plotting
      if (this.onProgressDataUpdate) {
        this.onProgressDataUpdate(iteration, fitness, convergence);
      } else {
        console.warn("[OPT DEBUG] No onProgressDataUpdate callback set!");
      }
    }

    // Notify UI with calculated percentage
    if (this.onProgressUpdate) {
      this.onProgressUpdate(
        typeof stage === "string" ? stage : "Optimization",
        typeof status === "string" ? status : "running",
        typeof details === "string" ? String(details) : "",
        calculatedPercentage,
      );
    }
  }

  private updateOptimizationStage(
    stage: string,
    status: string,
    details: string,
  ): void {
    const existingStageIndex = this.optimizationStages.findIndex(
      (s) => s.status === stage,
    );

    if (existingStageIndex >= 0) {
      // Update existing stage
      this.optimizationStages[existingStageIndex] = {
        ...this.optimizationStages[existingStageIndex],
        status,
        details,
        endTime: status === "completed" ? Date.now() : undefined,
      };
    } else {
      // Add new stage
      this.optimizationStages.push({
        status: stage,
        startTime: Date.now(),
        details,
      });
    }
  }

  async runOptimization(
    params: OptimizationParams,
  ): Promise<OptimizationResult> {
    if (this.isOptimizationRunning) {
      throw new Error("Optimization is already running");
    }

    this.isOptimizationRunning = true;
    this.currentOptimizationId = this.generateOptimizationId();
    this.optimizationStages = [];
    this.progressData = [];

    console.log("[OPTIMIZATION] 🚀 Starting optimization with params:", {
      num_filters: params.num_filters,
      algo: params.algo,
      loss: params.loss,
      has_target_frequencies: !!params.target_frequencies,
      target_frequencies_length: params.target_frequencies?.length,
      has_target_magnitudes: !!params.target_magnitudes,
      target_magnitudes_length: params.target_magnitudes?.length,
      has_curve_path: !!params.curve_path,
      curve_path: params.curve_path,
    });

    try {
      console.log("[OPTIMIZATION] 📞 Invoking backend run_optimization...");
      const result = (await invoke("run_optimization", {
        params,
      })) as OptimizationResult;
      console.log("[OPTIMIZATION] 📥 Backend returned result");

      if (result.success) {
        console.log(
          "[OPTIMIZATION] ✅ Backend returned success, processing result...",
        );
        console.log("[OPTIMIZATION] Result data:", {
          has_filter_params: !!result.filter_params,
          has_scores: !!(
            result.preference_score_before !== undefined &&
            result.preference_score_after !== undefined &&
            result.preference_score_before !== null &&
            result.preference_score_after !== null
          ),
          has_filter_response: !!result.filter_response,
          has_spin_details: !!result.spin_details,
          has_filter_plots: !!result.filter_plots,
          callback_registered: !!this.onOptimizationComplete,
        });

        // Store the optimization results for later use (e.g., APO export)
        if (result.filter_params) {
          this.lastFilterParams = result.filter_params;
          this.lastSampleRate = params.sample_rate;
          this.lastPeqModel = params.peq_model || "pk";
          this.lastLossType = params.loss || "flat";
          this.lastSpeakerName = params.speaker || null;
        }

        if (this.onOptimizationComplete) {
          console.log(
            "[OPTIMIZATION] 🔔 Invoking onOptimizationComplete callback...",
          );
          this.onOptimizationComplete(result);
          console.log("[OPTIMIZATION] ✅ Callback invoked successfully");
        } else {
          console.error(
            "[OPTIMIZATION] ❌ No onOptimizationComplete callback registered!",
          );
        }
      } else {
        const error = result.error_message || "Unknown optimization error";
        if (this.onOptimizationError) {
          this.onOptimizationError(error);
        }
      }

      return result;
    } catch (error) {
      console.error("[OPTIMIZATION] ❌ Optimization failed with error:", error);
      console.error("[OPTIMIZATION] Error details:", {
        type: typeof error,
        isError: error instanceof Error,
        message: error instanceof Error ? error.message : String(error),
        stack: error instanceof Error ? error.stack : undefined,
      });
      const errorMessage =
        error instanceof Error ? error.message : "Unknown error";

      if (this.onOptimizationError) {
        this.onOptimizationError(errorMessage);
      }

      throw error;
    } finally {
      this.isOptimizationRunning = false;
      this.currentOptimizationId = null;
    }
  }

  async cancelOptimization(): Promise<void> {
    if (!this.isOptimizationRunning || !this.currentOptimizationId) {
      return;
    }

    try {
      // Cancel via backend command
      await invoke("cancel_optimization");

      // Always perform local cleanup regardless of backend response
      this.isOptimizationRunning = false;
      this.currentOptimizationId = null;

      // Clean up progress listener
      if (this.progressUnlisten) {
        this.progressUnlisten();
        this.progressUnlisten = null;
      }
    } catch (error) {
      console.error("Failed to cancel optimization:", error);
      // Even if there's an error, we should still clean up local state
      this.isOptimizationRunning = false;
      this.currentOptimizationId = null;
      if (this.progressUnlisten) {
        this.progressUnlisten();
        this.progressUnlisten = null;
      }
      throw error;
    }
  }

  async extractOptimizationParams(
    formData: FormData,
  ): Promise<OptimizationParams> {
    const inputType = formData.get("input_source") as string;

    const baseParams: OptimizationParams = {
      num_filters:
        parseInt(formData.get("num_filters") as string) ||
        OPTIMIZATION_DEFAULTS.num_filters,
      sample_rate:
        parseInt(formData.get("sample_rate") as string) ||
        OPTIMIZATION_DEFAULTS.sample_rate,
      max_db:
        parseFloat(formData.get("max_db") as string) ||
        OPTIMIZATION_DEFAULTS.max_db,
      min_db:
        parseFloat(formData.get("min_db") as string) ||
        OPTIMIZATION_DEFAULTS.min_db,
      max_q:
        parseFloat(formData.get("max_q") as string) ||
        OPTIMIZATION_DEFAULTS.max_q,
      min_q:
        parseFloat(formData.get("min_q") as string) ||
        OPTIMIZATION_DEFAULTS.min_q,
      min_freq:
        parseFloat(formData.get("min_freq") as string) ||
        OPTIMIZATION_DEFAULTS.min_freq,
      max_freq:
        parseFloat(formData.get("max_freq") as string) ||
        OPTIMIZATION_DEFAULTS.max_freq,
      curve_name:
        (formData.get("curve_name") as string) ||
        OPTIMIZATION_DEFAULTS.curve_name,
      algo: (formData.get("algo") as string) || OPTIMIZATION_DEFAULTS.algo,
      population:
        parseInt(formData.get("population") as string) ||
        OPTIMIZATION_DEFAULTS.population,
      maxeval:
        parseInt(formData.get("maxeval") as string) ||
        OPTIMIZATION_DEFAULTS.maxeval,
      refine: formData.get("refine") === "on",
      local_algo:
        (formData.get("local_algo") as string) ||
        OPTIMIZATION_DEFAULTS.local_algo,
      min_spacing_oct:
        parseFloat(formData.get("min_spacing_oct") as string) ||
        OPTIMIZATION_DEFAULTS.min_spacing_oct,
      spacing_weight:
        parseFloat(formData.get("spacing_weight") as string) ||
        OPTIMIZATION_DEFAULTS.spacing_weight,
      smooth: formData.get("smooth") === "on",
      smooth_n:
        parseInt(formData.get("smooth_n") as string) ||
        OPTIMIZATION_DEFAULTS.smooth_n,
      loss: (formData.get("loss") as string) || OPTIMIZATION_DEFAULTS.loss,
      peq_model: (formData.get("peq_model") as PeqModel) || "pk",
      iir_hp_pk:
        formData.get("iir_hp_pk") === "on" ||
        formData.get("peq_model") === "hp-pk", // For backward compatibility
      tolerance:
        parseFloat(formData.get("tolerance") as string) ||
        OPTIMIZATION_DEFAULTS.tolerance,
      atolerance:
        parseFloat(formData.get("abs_tolerance") as string) ||
        OPTIMIZATION_DEFAULTS.abs_tolerance,
    };

    // Add input-source specific parameters
    // IMPORTANT: Only set the parameters for the selected input type
    // to avoid backend confusion
    if (inputType === "speaker") {
      // Speaker data from API
      baseParams.speaker = formData.get("speaker") as string;
      baseParams.version = formData.get("version") as string;
      baseParams.measurement = formData.get("measurement") as string;
      // Explicitly clear file params
      baseParams.curve_path = undefined;
      baseParams.target_path = undefined;
      baseParams.captured_frequencies = undefined;
      baseParams.captured_magnitudes = undefined;
    } else if (inputType === "headphone") {
      // Headphone data from file with target curve
      baseParams.curve_path = formData.get("headphone_curve_path") as string;
      const headphoneTarget = formData.get("headphone_target") as string;

      // Load target curve data from CSV file
      if (headphoneTarget) {
        console.log(
          "[OPTIMIZATION] Loading headphone target:",
          headphoneTarget,
        );
        const targetData = await this.loadHeadphoneTarget(headphoneTarget);
        if (targetData) {
          console.log("[OPTIMIZATION] Target data loaded:", {
            frequencies: targetData.frequencies.length,
            magnitudes: targetData.magnitudes.length,
            firstFreq: targetData.frequencies[0],
            lastFreq: targetData.frequencies[targetData.frequencies.length - 1],
          });
          baseParams.target_frequencies = targetData.frequencies;
          baseParams.target_magnitudes = targetData.magnitudes;
        } else {
          console.error(
            "[OPTIMIZATION] Failed to load headphone target:",
            headphoneTarget,
          );
        }
      }

      // Keep curve_name for informational purposes
      baseParams.curve_name = headphoneTarget || baseParams.curve_name;

      // Explicitly clear other params
      baseParams.target_path = undefined; // Don't use file path anymore
      baseParams.speaker = undefined;
      baseParams.version = undefined;
      baseParams.measurement = undefined;
      baseParams.captured_frequencies = undefined;
      baseParams.captured_magnitudes = undefined;
    } else if (inputType === "file") {
      baseParams.curve_path = formData.get("curve_path") as string;
      baseParams.target_path = formData.get("target_path") as string;
      // Explicitly clear API params
      baseParams.speaker = undefined;
      baseParams.version = undefined;
      baseParams.measurement = undefined;
      baseParams.captured_frequencies = undefined;
      baseParams.captured_magnitudes = undefined;
    } else if (inputType === "capture") {
      // Captured data will be added separately
      const capturedFreqs = this.getCapturedFrequencies();
      const capturedMags = this.getCapturedMagnitudes();
      if (capturedFreqs && capturedMags) {
        baseParams.captured_frequencies = capturedFreqs;
        baseParams.captured_magnitudes = capturedMags;
      }
      // Explicitly clear both API and file params
      baseParams.speaker = undefined;
      baseParams.version = undefined;
      baseParams.measurement = undefined;
      baseParams.curve_path = undefined;
      baseParams.target_path = undefined;
    }

    // Add DE-specific parameters if using DE algorithm
    if (baseParams.algo === "autoeq_de") {
      baseParams.strategy = (formData.get("strategy") as string) || "best1bin";
      baseParams.de_f = parseFloat(formData.get("de_f") as string) || 0.8;
      baseParams.de_cr = parseFloat(formData.get("de_cr") as string) || 0.9;
      baseParams.adaptive_weight_f =
        parseFloat(formData.get("adaptive_weight_f") as string) || 0.1;
      baseParams.adaptive_weight_cr =
        parseFloat(formData.get("adaptive_weight_cr") as string) || 0.1;
    }

    return baseParams;
  }

  // Methods to load headphone target curve data
  private async loadHeadphoneTarget(
    targetName: string,
  ): Promise<{ frequencies: number[]; magnitudes: number[] } | null> {
    try {
      const targetPath = `/headphone-targets/${targetName}.csv`;
      console.log("[OPTIMIZATION] Fetching target from:", targetPath);

      // Fetch the CSV file from the public directory (files in public/ are served at root)
      const response = await fetch(targetPath);

      console.log("[OPTIMIZATION] Fetch response:", {
        ok: response.ok,
        status: response.status,
        statusText: response.statusText,
      });

      if (!response.ok) {
        console.error(
          `[OPTIMIZATION] Failed to fetch target file: ${response.statusText}`,
        );
        return null;
      }

      const csvText = await response.text();
      console.log("[OPTIMIZATION] CSV text length:", csvText.length);
      const lines = csvText.trim().split("\n");
      console.log("[OPTIMIZATION] CSV lines:", lines.length);

      const frequencies: number[] = [];
      const magnitudes: number[] = [];

      // Parse CSV (skip header if present)
      for (let i = 0; i < lines.length; i++) {
        const line = lines[i].trim();
        if (!line || line.startsWith("#")) continue; // Skip empty lines and comments

        // Check if this might be a header line
        if (
          i === 0 &&
          (line.toLowerCase().includes("freq") ||
            line.toLowerCase().includes("hz") ||
            line.toLowerCase().includes("spl"))
        ) {
          continue; // Skip header
        }

        const parts = line.split(",").map((p) => p.trim());
        if (parts.length >= 2) {
          const freq = parseFloat(parts[0]);
          const mag = parseFloat(parts[1]);

          if (!isNaN(freq) && !isNaN(mag)) {
            frequencies.push(freq);
            magnitudes.push(mag);
          }
        }
      }

      console.log("[OPTIMIZATION] Parsed target data points:", {
        frequencies: frequencies.length,
        magnitudes: magnitudes.length,
      });

      return { frequencies, magnitudes };
    } catch (error) {
      console.error(
        `[OPTIMIZATION] Error loading headphone target ${targetName}:`,
        error,
      );
      return null;
    }
  }

  // Methods to store captured data from audio capture
  setCapturedData(frequencies: number[], magnitudes: number[]): void {
    this.capturedFrequencies = [...frequencies];
    this.capturedMagnitudes = [...magnitudes];
  }

  clearCapturedData(): void {
    this.capturedFrequencies = null;
    this.capturedMagnitudes = null;
  }

  hasCapturedData(): boolean {
    return (
      this.capturedFrequencies !== null && this.capturedMagnitudes !== null
    );
  }

  private getCapturedFrequencies(): number[] | null {
    return this.capturedFrequencies;
  }

  private getCapturedMagnitudes(): number[] | null {
    return this.capturedMagnitudes;
  }

  private generateOptimizationId(): string {
    return `opt_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
  }

  // Getters for state
  isRunning(): boolean {
    return this.isOptimizationRunning;
  }

  getCurrentOptimizationId(): string | null {
    return this.currentOptimizationId;
  }

  getOptimizationStages(): OptimizationStage[] {
    return [...this.optimizationStages];
  }

  getProgressData(): ProgressData[] {
    return [...this.progressData];
  }

  // Getters for optimization results (for APO export)
  getFilterParams(): number[] | null {
    return this.lastFilterParams ? [...this.lastFilterParams] : null;
  }

  getSampleRate(): number | null {
    return this.lastSampleRate;
  }

  getPeqModel(): PeqModel | null {
    return this.lastPeqModel;
  }

  getLossType(): string | null {
    return this.lastLossType;
  }

  getSpeakerName(): string | null {
    return this.lastSpeakerName;
  }

  hasOptimizationResult(): boolean {
    return this.lastFilterParams !== null && this.lastSampleRate !== null;
  }

  // Cleanup
  destroy(): void {
    if (this.progressUnlisten) {
      this.progressUnlisten();
      this.progressUnlisten = null;
    }

    if (this.isOptimizationRunning) {
      this.cancelOptimization().catch(console.error);
    }
  }
}
