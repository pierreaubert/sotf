// Storage module for persisting captured audio data

export interface StoredCapture {
  id: string;
  timestamp: Date;
  deviceName: string;
  signalType: "sweep" | "white" | "pink";
  duration: number;
  sampleRate: number;
  outputChannel: string;
  frequencies: number[];
  rawMagnitudes: number[];
  smoothedMagnitudes: number[];
  rawPhase: number[]; // Phase data in degrees
  smoothedPhase: number[]; // Smoothed phase data in degrees
  name: string; // Display name
}

export interface CaptureStorageStats {
  totalCaptures: number;
  oldestCapture: Date | null;
  newestCapture: Date | null;
  totalSizeKB: number;
}

export class CaptureStorage {
  private static readonly STORAGE_KEY = "autoeq_captured_curves";
  private static readonly MAX_CAPTURES = 10;
  private static readonly VERSION = "1.0";

  /**
   * Save a capture to storage
   */
  public static saveCapture(
    captureData: Omit<StoredCapture, "id" | "name">,
  ): string {
    const captures = this.getAllCaptures();

    // Generate unique ID
    const id = this.generateId();

    // Generate display name
    const name = this.generateDisplayName(captureData);

    // Create stored capture
    const storedCapture: StoredCapture = {
      ...captureData,
      id,
      name,
    };

    // Add to beginning of array (most recent first)
    captures.unshift(storedCapture);

    // Enforce storage limit
    if (captures.length > this.MAX_CAPTURES) {
      const removed = captures.splice(this.MAX_CAPTURES);
      console.log(
        `Removed ${removed.length} old captures to maintain storage limit`,
      );
    }

    // Save to localStorage
    this.saveToStorage(captures);

    console.log(`Saved capture ${id}: ${name}`);
    return id;
  }

  /**
   * Get all captures from storage
   */
  public static getAllCaptures(): StoredCapture[] {
    try {
      const data = localStorage.getItem(this.STORAGE_KEY);
      if (!data) {
        return [];
      }

      const parsed = JSON.parse(data);

      // Version check
      if (parsed.version !== this.VERSION) {
        console.log("Storage version mismatch, clearing old data");
        this.clearAll();
        return [];
      }

      // Convert timestamp strings back to Date objects
      const captures = parsed.captures.map(
        (capture: StoredCapture & { timestamp: string | Date }) => ({
          ...capture,
          timestamp: new Date(capture.timestamp),
        }),
      );

      return captures;
    } catch (error) {
      console.error("Error loading captures from storage:", error);
      return [];
    }
  }

  /**
   * Get a specific capture by ID
   */
  public static getCapture(id: string): StoredCapture | null {
    const captures = this.getAllCaptures();
    return captures.find((capture) => capture.id === id) || null;
  }

  /**
   * Delete a specific capture
   */
  public static deleteCapture(id: string): boolean {
    const captures = this.getAllCaptures();
    const index = captures.findIndex((capture) => capture.id === id);

    if (index === -1) {
      return false;
    }

    const deleted = captures.splice(index, 1)[0];
    this.saveToStorage(captures);

    console.log(`Deleted capture ${id}: ${deleted.name}`);
    return true;
  }

  /**
   * Clear all captures
   */
  public static clearAll(): void {
    localStorage.removeItem(this.STORAGE_KEY);
    console.log("Cleared all captured curves from storage");
  }

  /**
   * Get storage statistics
   */
  public static getStats(): CaptureStorageStats {
    const captures = this.getAllCaptures();

    if (captures.length === 0) {
      return {
        totalCaptures: 0,
        oldestCapture: null,
        newestCapture: null,
        totalSizeKB: 0,
      };
    }

    // Calculate storage size (rough estimate)
    const dataString = localStorage.getItem(this.STORAGE_KEY) || "";
    const sizeKB = Math.round((dataString.length * 2) / 1024); // UTF-16 = 2 bytes per char

    // Sort by timestamp to find oldest/newest
    const sorted = [...captures].sort(
      (a, b) => a.timestamp.getTime() - b.timestamp.getTime(),
    );

    return {
      totalCaptures: captures.length,
      oldestCapture: sorted[0].timestamp,
      newestCapture: sorted[sorted.length - 1].timestamp,
      totalSizeKB: sizeKB,
    };
  }

  /**
   * Get captures suitable for curve menu display
   */
  public static getCapturesForMenu(): Array<{
    id: string;
    name: string;
    timestamp: Date;
    preview: string;
  }> {
    const captures = this.getAllCaptures();

    return captures.map((capture) => ({
      id: capture.id,
      name: capture.name,
      timestamp: capture.timestamp,
      preview: this.generatePreview(capture),
    }));
  }

  /**
   * Convert capture to optimization format
   */
  public static toOptimizationFormat(
    id: string,
  ): { frequencies: number[]; magnitudes: number[] } | null {
    const capture = this.getCapture(id);
    if (!capture) {
      return null;
    }

    return {
      frequencies: [...capture.frequencies],
      magnitudes: [...capture.smoothedMagnitudes], // Use smoothed data for optimization
    };
  }

  /**
   * Export capture as CSV data (without triggering download)
   */
  public static exportCaptureData(id: string): string | null {
    const capture = this.getCapture(id);
    if (!capture) {
      return null;
    }

    // Use the CSV exporter
    const exportData = {
      frequencies: capture.frequencies,
      rawMagnitudes: capture.rawMagnitudes,
      smoothedMagnitudes: capture.smoothedMagnitudes,
      metadata: {
        timestamp: capture.timestamp,
        deviceName: capture.deviceName,
        signalType: capture.signalType,
        duration: capture.duration,
        sampleRate: capture.sampleRate,
        outputChannel: capture.outputChannel,
      },
    };

    return this.generateCSV(exportData);
  }

  // Private helper methods

  private static generateId(): string {
    return (
      "capture_" + Date.now() + "_" + Math.random().toString(36).substr(2, 9)
    );
  }

  private static generateDisplayName(
    capture: Omit<StoredCapture, "id" | "name">,
  ): string {
    const date = capture.timestamp.toLocaleDateString();
    const time = capture.timestamp.toLocaleTimeString([], {
      hour: "2-digit",
      minute: "2-digit",
    });
    const channel =
      capture.outputChannel === "both"
        ? "Stereo"
        : capture.outputChannel === "left"
          ? "Left"
          : capture.outputChannel === "right"
            ? "Right"
            : "Default";
    const signal =
      capture.signalType.charAt(0).toUpperCase() + capture.signalType.slice(1);

    return `${date} ${time} - ${channel} (${signal})`;
  }

  private static generatePreview(capture: StoredCapture): string {
    const freqRange = `${Math.round(Math.min(...capture.frequencies))}Hz - ${Math.round(Math.max(...capture.frequencies))}Hz`;
    const magRange = `${Math.round(Math.min(...capture.smoothedMagnitudes))}dB to ${Math.round(Math.max(...capture.smoothedMagnitudes))}dB`;
    const points = capture.frequencies.length;

    return `${points} points, ${freqRange}, Range: ${magRange}`;
  }

  private static saveToStorage(captures: StoredCapture[]): void {
    const data = {
      version: this.VERSION,
      savedAt: new Date().toISOString(),
      captures,
    };

    try {
      const jsonString = JSON.stringify(data);
      localStorage.setItem(this.STORAGE_KEY, jsonString);
    } catch (error) {
      console.error("Error saving captures to storage:", error);
      // Handle storage quota exceeded
      if (error instanceof Error && error.name === "QuotaExceededError") {
        console.log("Storage quota exceeded, removing oldest captures");
        // Try to save with fewer captures
        if (captures.length > 3) {
          const reducedCaptures = captures.slice(
            0,
            Math.floor(captures.length / 2),
          );
          const reducedData = { ...data, captures: reducedCaptures };
          try {
            localStorage.setItem(this.STORAGE_KEY, JSON.stringify(reducedData));
            console.log(
              `Reduced storage to ${reducedCaptures.length} captures`,
            );
          } catch (secondError) {
            console.error("Failed to save even reduced data:", secondError);
            this.clearAll();
          }
        }
      }
    }
  }

  private static generateCSV(data: {
    frequencies: number[];
    rawMagnitudes: number[];
    smoothedMagnitudes: number[];
    rawPhase?: number[];
    smoothedPhase?: number[];
    metadata: {
      timestamp: Date;
      deviceName: string;
      signalType: string;
      duration: number;
      sampleRate: number;
      outputChannel: string;
    };
  }): string {
    const lines: string[] = [];

    // Add header comments with metadata
    lines.push("# AutoEQ Audio Capture Data");
    lines.push(`# Capture Date: ${data.metadata.timestamp.toISOString()}`);
    lines.push(`# Device: ${data.metadata.deviceName}`);
    lines.push(`# Signal Type: ${data.metadata.signalType}`);
    lines.push(`# Duration: ${data.metadata.duration}s`);
    lines.push(`# Sample Rate: ${data.metadata.sampleRate}Hz`);
    lines.push(`# Output Channel: ${data.metadata.outputChannel}`);
    lines.push(`# Generated by AutoEQ App`);
    lines.push("");

    // Column headers
    lines.push(
      "Frequency(Hz),Raw_SPL(dB),Smoothed_SPL(dB),Raw_Phase(deg),Smoothed_Phase(deg)",
    );

    // Data rows
    const length = Math.min(
      data.frequencies.length,
      data.rawMagnitudes.length,
      data.smoothedMagnitudes.length,
    );

    for (let i = 0; i < length; i++) {
      const freq = data.frequencies[i].toFixed(2);
      const rawMag = data.rawMagnitudes[i].toFixed(3);
      const smoothedMag = data.smoothedMagnitudes[i].toFixed(3);
      const rawPhase =
        data.rawPhase && data.rawPhase[i] !== undefined
          ? data.rawPhase[i].toFixed(1)
          : "0.0";
      const smoothedPhase =
        data.smoothedPhase && data.smoothedPhase[i] !== undefined
          ? data.smoothedPhase[i].toFixed(1)
          : "0.0";
      lines.push(
        `${freq},${rawMag},${smoothedMag},${rawPhase},${smoothedPhase}`,
      );
    }

    return lines.join("\n");
  }

  /**
   * Get captures grouped by output channel
   */
  public static getCapturesByChannel(): Map<string, StoredCapture[]> {
    const captures = this.getAllCaptures();
    const grouped = new Map<string, StoredCapture[]>();

    for (const capture of captures) {
      const channel = capture.outputChannel || "default";
      if (!grouped.has(channel)) {
        grouped.set(channel, []);
      }
      grouped.get(channel)!.push(capture);
    }

    return grouped;
  }

  /**
   * Get the most recent capture for a specific channel
   */
  public static getLatestCaptureForChannel(
    channel: string,
  ): StoredCapture | null {
    const captures = this.getAllCaptures();
    const channelCaptures = captures.filter((c) => c.outputChannel === channel);

    if (channelCaptures.length === 0) {
      return null;
    }

    // Already sorted by timestamp (newest first)
    return channelCaptures[0];
  }

  /**
   * Check storage health and perform maintenance
   */
  public static performMaintenance(): void {
    const captures = this.getAllCaptures();
    let changed = false;

    // Remove captures with invalid data
    const validCaptures = captures.filter((capture) => {
      const isValid =
        capture.frequencies?.length > 0 &&
        capture.rawMagnitudes?.length === capture.frequencies.length &&
        capture.smoothedMagnitudes?.length === capture.frequencies.length &&
        capture.timestamp instanceof Date &&
        !isNaN(capture.timestamp.getTime());

      if (!isValid) {
        console.log(`Removing invalid capture: ${capture.id}`);
        changed = true;
      }

      return isValid;
    });

    // Sort by timestamp (newest first)
    validCaptures.sort((a, b) => b.timestamp.getTime() - a.timestamp.getTime());

    // Enforce limits
    if (validCaptures.length > this.MAX_CAPTURES) {
      const excess = validCaptures.splice(this.MAX_CAPTURES);
      console.log(
        `Removed ${excess.length} excess captures during maintenance`,
      );
      changed = true;
    }

    if (changed) {
      this.saveToStorage(validCaptures);
      console.log("Storage maintenance completed");
    }
  }
}
