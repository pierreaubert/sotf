/**
 * Tests for capture-storage module
 * Tests localStorage persistence, capture management, and data validation
 */

import { describe, it, expect, beforeEach, afterEach } from "vitest";
import {
  CaptureStorage,
  type StoredCapture,
} from "@audio-capture/capture-storage";

describe("capture-storage", () => {
  // Mock localStorage
  let mockStorage: Record<string, string> = {};

  beforeEach(() => {
    // Reset mock storage before each test
    mockStorage = {};

    // Mock localStorage
    global.localStorage = {
      getItem: (key: string) => mockStorage[key] || null,
      setItem: (key: string, value: string) => {
        mockStorage[key] = value;
      },
      removeItem: (key: string) => {
        delete mockStorage[key];
      },
      clear: () => {
        mockStorage = {};
      },
      length: 0,
      key: () => null,
    };
  });

  afterEach(() => {
    CaptureStorage.clearAll();
  });

  describe("saveCapture", () => {
    it("should save a capture and return an ID", () => {
      const captureData = {
        timestamp: new Date(),
        deviceName: "Test Device",
        signalType: "sweep" as const,
        duration: 10,
        sampleRate: 48000,
        outputChannel: "both",
        frequencies: [100, 1000, 10000],
        rawMagnitudes: [0, -3, -6],
        smoothedMagnitudes: [0, -3, -6],
        rawPhase: [0, 45, 90],
        smoothedPhase: [0, 45, 90],
      };

      const id = CaptureStorage.saveCapture(captureData);

      expect(id).toBeDefined();
      expect(id.length).toBeGreaterThan(0);

      const retrieved = CaptureStorage.getCapture(id);
      expect(retrieved).not.toBeNull();
      expect(retrieved?.deviceName).toBe("Test Device");
    });

    it("should generate unique IDs for multiple captures", () => {
      const captureData = {
        timestamp: new Date(),
        deviceName: "Test Device",
        signalType: "sweep" as const,
        duration: 10,
        sampleRate: 48000,
        outputChannel: "both",
        frequencies: [100],
        rawMagnitudes: [0],
        smoothedMagnitudes: [0],
        rawPhase: [0],
        smoothedPhase: [0],
      };

      const id1 = CaptureStorage.saveCapture(captureData);
      const id2 = CaptureStorage.saveCapture(captureData);
      const id3 = CaptureStorage.saveCapture(captureData);

      expect(id1).not.toBe(id2);
      expect(id2).not.toBe(id3);
      expect(id1).not.toBe(id3);
    });

    it("should enforce storage limit", () => {
      const captureData = {
        timestamp: new Date(),
        deviceName: "Test Device",
        signalType: "sweep" as const,
        duration: 10,
        sampleRate: 48000,
        outputChannel: "both",
        frequencies: [100],
        rawMagnitudes: [0],
        smoothedMagnitudes: [0],
        rawPhase: [0],
        smoothedPhase: [0],
      };

      // Save 15 captures (max is 10)
      const ids: string[] = [];
      for (let i = 0; i < 15; i++) {
        ids.push(
          CaptureStorage.saveCapture({
            ...captureData,
            deviceName: `Device ${i}`,
          }),
        );
      }

      const allCaptures = CaptureStorage.getAllCaptures();
      expect(allCaptures.length).toBeLessThanOrEqual(10);

      // Most recent should be kept
      expect(CaptureStorage.getCapture(ids[14])).not.toBeNull();
      // Oldest should be removed
      expect(CaptureStorage.getCapture(ids[0])).toBeNull();
    });

    it("should generate display names", () => {
      const captureData = {
        timestamp: new Date(),
        deviceName: "Test Device",
        signalType: "sweep" as const,
        duration: 10,
        sampleRate: 48000,
        outputChannel: "both",
        frequencies: [100],
        rawMagnitudes: [0],
        smoothedMagnitudes: [0],
        rawPhase: [0],
        smoothedPhase: [0],
      };

      const id = CaptureStorage.saveCapture(captureData);
      const capture = CaptureStorage.getCapture(id);

      expect(capture?.name).toBeDefined();
      expect(capture?.name.length).toBeGreaterThan(0);
    });
  });

  describe("getAllCaptures", () => {
    it("should return empty array when no captures exist", () => {
      const captures = CaptureStorage.getAllCaptures();
      expect(captures).toEqual([]);
    });

    it("should return all saved captures", () => {
      const captureData = {
        timestamp: new Date(),
        deviceName: "Test Device",
        signalType: "sweep" as const,
        duration: 10,
        sampleRate: 48000,
        outputChannel: "both",
        frequencies: [100],
        rawMagnitudes: [0],
        smoothedMagnitudes: [0],
        rawPhase: [0],
        smoothedPhase: [0],
      };

      CaptureStorage.saveCapture(captureData);
      CaptureStorage.saveCapture(captureData);

      const captures = CaptureStorage.getAllCaptures();
      expect(captures.length).toBe(2);
    });

    it("should handle corrupted storage gracefully", () => {
      mockStorage["autoeq_captured_curves"] = "invalid json";

      const captures = CaptureStorage.getAllCaptures();
      expect(captures).toEqual([]);
    });

    it("should clear old data on version mismatch", () => {
      mockStorage["autoeq_captured_curves"] = JSON.stringify({
        version: "0.9",
        captures: [{ id: "old" }],
      });

      const captures = CaptureStorage.getAllCaptures();
      expect(captures).toEqual([]);
    });
  });

  describe("getCapture", () => {
    it("should retrieve a specific capture by ID", () => {
      const captureData = {
        timestamp: new Date(),
        deviceName: "Test Device",
        signalType: "sweep" as const,
        duration: 10,
        sampleRate: 48000,
        outputChannel: "both",
        frequencies: [100],
        rawMagnitudes: [0],
        smoothedMagnitudes: [0],
        rawPhase: [0],
        smoothedPhase: [0],
      };

      const id = CaptureStorage.saveCapture(captureData);
      const retrieved = CaptureStorage.getCapture(id);

      expect(retrieved).not.toBeNull();
      expect(retrieved?.id).toBe(id);
      expect(retrieved?.deviceName).toBe("Test Device");
    });

    it("should return null for non-existent ID", () => {
      const retrieved = CaptureStorage.getCapture("non-existent-id");
      expect(retrieved).toBeNull();
    });
  });

  describe("deleteCapture", () => {
    it("should delete a capture and return true", () => {
      const captureData = {
        timestamp: new Date(),
        deviceName: "Test Device",
        signalType: "sweep" as const,
        duration: 10,
        sampleRate: 48000,
        outputChannel: "both",
        frequencies: [100],
        rawMagnitudes: [0],
        smoothedMagnitudes: [0],
        rawPhase: [0],
        smoothedPhase: [0],
      };

      const id = CaptureStorage.saveCapture(captureData);
      const result = CaptureStorage.deleteCapture(id);

      expect(result).toBe(true);
      expect(CaptureStorage.getCapture(id)).toBeNull();
    });

    it("should return false for non-existent ID", () => {
      const result = CaptureStorage.deleteCapture("non-existent");
      expect(result).toBe(false);
    });

    it("should maintain other captures after deletion", () => {
      const captureData = {
        timestamp: new Date(),
        deviceName: "Test Device",
        signalType: "sweep" as const,
        duration: 10,
        sampleRate: 48000,
        outputChannel: "both",
        frequencies: [100],
        rawMagnitudes: [0],
        smoothedMagnitudes: [0],
        rawPhase: [0],
        smoothedPhase: [0],
      };

      const id1 = CaptureStorage.saveCapture({
        ...captureData,
        deviceName: "Device 1",
      });
      const id2 = CaptureStorage.saveCapture({
        ...captureData,
        deviceName: "Device 2",
      });

      CaptureStorage.deleteCapture(id1);

      expect(CaptureStorage.getCapture(id2)).not.toBeNull();
      expect(CaptureStorage.getAllCaptures().length).toBe(1);
    });
  });

  describe("clearAll", () => {
    it("should remove all captures", () => {
      const captureData = {
        timestamp: new Date(),
        deviceName: "Test Device",
        signalType: "sweep" as const,
        duration: 10,
        sampleRate: 48000,
        outputChannel: "both",
        frequencies: [100],
        rawMagnitudes: [0],
        smoothedMagnitudes: [0],
        rawPhase: [0],
        smoothedPhase: [0],
      };

      CaptureStorage.saveCapture(captureData);
      CaptureStorage.saveCapture(captureData);

      CaptureStorage.clearAll();

      expect(CaptureStorage.getAllCaptures()).toEqual([]);
    });
  });

  describe("getStats", () => {
    it("should return zero stats for empty storage", () => {
      const stats = CaptureStorage.getStats();

      expect(stats.totalCaptures).toBe(0);
      expect(stats.oldestCapture).toBeNull();
      expect(stats.newestCapture).toBeNull();
      expect(stats.totalSizeKB).toBe(0);
    });

    it("should calculate stats correctly", () => {
      const captureData = {
        timestamp: new Date(),
        deviceName: "Test Device",
        signalType: "sweep" as const,
        duration: 10,
        sampleRate: 48000,
        outputChannel: "both",
        frequencies: [100],
        rawMagnitudes: [0],
        smoothedMagnitudes: [0],
        rawPhase: [0],
        smoothedPhase: [0],
      };

      CaptureStorage.saveCapture(captureData);
      CaptureStorage.saveCapture({
        ...captureData,
        timestamp: new Date(Date.now() + 1000),
      });

      const stats = CaptureStorage.getStats();

      expect(stats.totalCaptures).toBe(2);
      expect(stats.oldestCapture).toBeInstanceOf(Date);
      expect(stats.newestCapture).toBeInstanceOf(Date);
      expect(stats.totalSizeKB).toBeGreaterThan(0);
    });
  });

  describe("toOptimizationFormat", () => {
    it("should convert capture to optimization format", () => {
      const captureData = {
        timestamp: new Date(),
        deviceName: "Test Device",
        signalType: "sweep" as const,
        duration: 10,
        sampleRate: 48000,
        outputChannel: "both",
        frequencies: [100, 1000, 10000],
        rawMagnitudes: [0, -3, -6],
        smoothedMagnitudes: [1, -2, -5],
        rawPhase: [0, 45, 90],
        smoothedPhase: [0, 45, 90],
      };

      const id = CaptureStorage.saveCapture(captureData);
      const format = CaptureStorage.toOptimizationFormat(id);

      expect(format).not.toBeNull();
      expect(format?.frequencies).toEqual([100, 1000, 10000]);
      expect(format?.magnitudes).toEqual([1, -2, -5]); // Uses smoothed data
    });

    it("should return null for non-existent capture", () => {
      const format = CaptureStorage.toOptimizationFormat("non-existent");
      expect(format).toBeNull();
    });
  });
});
