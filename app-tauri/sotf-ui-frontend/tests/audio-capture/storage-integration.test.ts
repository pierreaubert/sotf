/**
 * Integration tests for CaptureStorage usage in main.ts
 * Tests verify the deduplication of storage between src-audio-capture and src-ui
 * and prevent regressions in storage functionality
 */

import { describe, it, expect, beforeEach, afterEach } from "vitest";
import {
  CaptureStorage,
  type StoredCapture,
} from "@audio-capture/capture-storage";

describe("CaptureStorage Integration Tests", () => {
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

  describe("Main.ts Integration", () => {
    it("should save capture with smoothed data after capture completion", () => {
      // Simulate capture completion
      const captureData = {
        timestamp: new Date(),
        deviceName: "Test Microphone",
        signalType: "sweep" as const,
        duration: 10,
        sampleRate: 48000,
        outputChannel: "both",
        frequencies: [100, 1000, 10000],
        rawMagnitudes: [0, -3, -6],
        smoothedMagnitudes: [0.2, -2.8, -5.9],
        rawPhase: [0, 45, 90],
        smoothedPhase: [0.1, 44.9, 89.8],
      };

      // Save capture as main.ts would
      const id = CaptureStorage.saveCapture(captureData);

      // Verify it was saved
      expect(id).toBeDefined();
      expect(id.length).toBeGreaterThan(0);

      // Verify data structure
      const retrieved = CaptureStorage.getCapture(id);
      expect(retrieved).not.toBeNull();
      expect(retrieved?.deviceName).toBe("Test Microphone");
      expect(retrieved?.frequencies).toEqual([100, 1000, 10000]);
      expect(retrieved?.rawMagnitudes).toEqual([0, -3, -6]);
      expect(retrieved?.smoothedMagnitudes).toEqual([0.2, -2.8, -5.9]);
      expect(retrieved?.rawPhase).toEqual([0, 45, 90]);
      expect(retrieved?.smoothedPhase).toEqual([0.1, 44.9, 89.8]);
    });

    it("should load capture data in format expected by main.ts", () => {
      // Save a capture
      const captureData = {
        timestamp: new Date(),
        deviceName: "Test Device",
        signalType: "sweep" as const,
        duration: 15,
        sampleRate: 96000,
        outputChannel: "left",
        frequencies: [20, 200, 2000, 20000],
        rawMagnitudes: [0, -3, -6, -12],
        smoothedMagnitudes: [0.1, -2.9, -5.8, -11.7],
        rawPhase: [0, 30, 60, 90],
        smoothedPhase: [0, 30, 60, 90],
      };

      const id = CaptureStorage.saveCapture(captureData);
      const retrieved = CaptureStorage.getCapture(id);

      // Verify main.ts can extract expected data structure
      expect(retrieved).not.toBeNull();
      if (retrieved) {
        // Simulate main.ts loadSweep function
        const currentCaptureData = {
          frequencies: retrieved.frequencies,
          magnitudes: retrieved.rawMagnitudes,
          phases: retrieved.rawPhase,
          metadata: {
            timestamp: new Date(retrieved.timestamp),
            deviceName: retrieved.deviceName,
            signalType: retrieved.signalType,
            duration: retrieved.duration,
            sampleRate: retrieved.sampleRate,
            outputChannel: retrieved.outputChannel,
          },
        };

        expect(currentCaptureData.frequencies).toEqual([20, 200, 2000, 20000]);
        expect(currentCaptureData.magnitudes).toEqual([0, -3, -6, -12]);
        expect(currentCaptureData.phases).toEqual([0, 30, 60, 90]);
        expect(currentCaptureData.metadata.deviceName).toBe("Test Device");
        expect(currentCaptureData.metadata.sampleRate).toBe(96000);
      }
    });

    it("should export capture data in format expected by CSVExporter", () => {
      const captureData = {
        timestamp: new Date(),
        deviceName: "Export Test Device",
        signalType: "white" as const,
        duration: 10,
        sampleRate: 48000,
        outputChannel: "both",
        frequencies: [100, 1000],
        rawMagnitudes: [0, -3],
        smoothedMagnitudes: [0.1, -2.9],
        rawPhase: [0, 45],
        smoothedPhase: [0.1, 44.8],
      };

      const id = CaptureStorage.saveCapture(captureData);
      const retrieved = CaptureStorage.getCapture(id);

      // Simulate main.ts exportSweep function
      expect(retrieved).not.toBeNull();
      if (retrieved) {
        const exportData = {
          frequencies: retrieved.frequencies,
          rawMagnitudes: retrieved.rawMagnitudes,
          smoothedMagnitudes: retrieved.smoothedMagnitudes,
          rawPhase: retrieved.rawPhase,
          smoothedPhase: retrieved.smoothedPhase,
          metadata: {
            timestamp: new Date(retrieved.timestamp),
            deviceName: retrieved.deviceName,
            signalType: retrieved.signalType,
            duration: retrieved.duration,
            sampleRate: retrieved.sampleRate,
            outputChannel: retrieved.outputChannel,
          },
        };

        expect(exportData.frequencies).toEqual([100, 1000]);
        expect(exportData.rawMagnitudes).toEqual([0, -3]);
        expect(exportData.smoothedMagnitudes).toEqual([0.1, -2.9]);
        expect(exportData.metadata.deviceName).toBe("Export Test Device");
      }
    });

    it("should delete capture by string ID", () => {
      const captureData = {
        timestamp: new Date(),
        deviceName: "Delete Test",
        signalType: "pink" as const,
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
      expect(CaptureStorage.getCapture(id)).not.toBeNull();

      // Delete using string ID (as main.ts does)
      const result = CaptureStorage.deleteCapture(id);
      expect(result).toBe(true);
      expect(CaptureStorage.getCapture(id)).toBeNull();
    });

    it("should list all captures for recall modal", () => {
      // Save multiple captures
      const captures = [
        {
          timestamp: new Date(Date.now() - 3000),
          deviceName: "Device 1",
          signalType: "sweep" as const,
          duration: 10,
          sampleRate: 48000,
          outputChannel: "both",
          frequencies: [100],
          rawMagnitudes: [0],
          smoothedMagnitudes: [0],
          rawPhase: [0],
          smoothedPhase: [0],
        },
        {
          timestamp: new Date(Date.now() - 2000),
          deviceName: "Device 2",
          signalType: "white" as const,
          duration: 15,
          sampleRate: 96000,
          outputChannel: "left",
          frequencies: [200],
          rawMagnitudes: [-3],
          smoothedMagnitudes: [-2.9],
          rawPhase: [45],
          smoothedPhase: [45],
        },
        {
          timestamp: new Date(Date.now() - 1000),
          deviceName: "Device 3",
          signalType: "pink" as const,
          duration: 20,
          sampleRate: 44100,
          outputChannel: "right",
          frequencies: [300],
          rawMagnitudes: [-6],
          smoothedMagnitudes: [-5.8],
          rawPhase: [90],
          smoothedPhase: [89.5],
        },
      ];

      captures.forEach((capture) => CaptureStorage.saveCapture(capture));

      // Simulate main.ts loadSweepsList function
      const allCaptures = CaptureStorage.getAllCaptures();
      expect(allCaptures.length).toBe(3);

      // Verify captures are sorted by timestamp (newest first)
      expect(allCaptures[0].deviceName).toBe("Device 3");
      expect(allCaptures[1].deviceName).toBe("Device 2");
      expect(allCaptures[2].deviceName).toBe("Device 1");

      // Verify each capture has required fields for UI display
      allCaptures.forEach((capture) => {
        expect(capture.id).toBeDefined();
        expect(capture.name).toBeDefined();
        expect(capture.timestamp).toBeDefined();
        expect(capture.deviceName).toBeDefined();
        expect(capture.signalType).toBeDefined();
        expect(capture.duration).toBeDefined();
        expect(capture.frequencies.length).toBeGreaterThan(0);
      });
    });

    it("should clear all captures", () => {
      // Save multiple captures
      for (let i = 0; i < 5; i++) {
        CaptureStorage.saveCapture({
          timestamp: new Date(),
          deviceName: `Device ${i}`,
          signalType: "sweep" as const,
          duration: 10,
          sampleRate: 48000,
          outputChannel: "both",
          frequencies: [100],
          rawMagnitudes: [0],
          smoothedMagnitudes: [0],
          rawPhase: [0],
          smoothedPhase: [0],
        });
      }

      expect(CaptureStorage.getAllCaptures().length).toBe(5);

      // Simulate main.ts clearAllSweeps function
      CaptureStorage.clearAll();

      expect(CaptureStorage.getAllCaptures().length).toBe(0);
    });
  });

  describe("Cross-Application Compatibility", () => {
    it("should be compatible with src-ui CaptureStorage usage", () => {
      // Simulate src-ui saving a capture via CaptureModalManager
      const uiCaptureData = {
        timestamp: new Date(),
        deviceName: "UI Test Device",
        signalType: "sweep" as const,
        duration: 10,
        sampleRate: 48000,
        outputChannel: "both",
        frequencies: [100, 1000, 10000],
        rawMagnitudes: [0, -3, -6],
        smoothedMagnitudes: [0.1, -2.9, -5.9],
        rawPhase: [0, 45, 90],
        smoothedPhase: [0, 45, 90],
      };

      const id = CaptureStorage.saveCapture(uiCaptureData);

      // Verify src-audio-capture can read it
      const retrieved = CaptureStorage.getCapture(id);
      expect(retrieved).not.toBeNull();
      expect(retrieved?.deviceName).toBe("UI Test Device");
      expect(retrieved?.frequencies).toEqual([100, 1000, 10000]);

      // Verify main.ts can load it in its expected format
      if (retrieved) {
        const mainTsFormat = {
          frequencies: retrieved.frequencies,
          magnitudes: retrieved.rawMagnitudes,
          phases: retrieved.rawPhase,
          metadata: {
            timestamp: new Date(retrieved.timestamp),
            deviceName: retrieved.deviceName,
            signalType: retrieved.signalType,
            duration: retrieved.duration,
            sampleRate: retrieved.sampleRate,
            outputChannel: retrieved.outputChannel,
          },
        };

        expect(mainTsFormat.frequencies).toEqual([100, 1000, 10000]);
        expect(mainTsFormat.magnitudes).toEqual([0, -3, -6]);
      }
    });

    it("should share storage between src-audio-capture and src-ui", () => {
      // Simulate src-audio-capture saving a capture
      const audioCaptureData = {
        timestamp: new Date(),
        deviceName: "Audio Capture Device",
        signalType: "sweep" as const,
        duration: 10,
        sampleRate: 48000,
        outputChannel: "left",
        frequencies: [100, 1000],
        rawMagnitudes: [0, -3],
        smoothedMagnitudes: [0.1, -2.9],
        rawPhase: [0, 45],
        smoothedPhase: [0, 45],
      };

      CaptureStorage.saveCapture(audioCaptureData);

      // Simulate src-ui saving a capture
      const uiCaptureData = {
        timestamp: new Date(Date.now() + 1000),
        deviceName: "UI Device",
        signalType: "white" as const,
        duration: 15,
        sampleRate: 96000,
        outputChannel: "right",
        frequencies: [200, 2000],
        rawMagnitudes: [-1, -4],
        smoothedMagnitudes: [-0.9, -3.8],
        rawPhase: [30, 60],
        smoothedPhase: [30, 60],
      };

      CaptureStorage.saveCapture(uiCaptureData);

      // Both should see all captures
      const allCaptures = CaptureStorage.getAllCaptures();
      expect(allCaptures.length).toBe(2);

      // Verify both are accessible
      const deviceNames = allCaptures.map((c) => c.deviceName);
      expect(deviceNames).toContain("Audio Capture Device");
      expect(deviceNames).toContain("UI Device");
    });
  });

  describe("Storage Limit and Cleanup", () => {
    it("should enforce storage limit of 10 captures", () => {
      // Save 15 captures
      const ids: string[] = [];
      for (let i = 0; i < 15; i++) {
        const id = CaptureStorage.saveCapture({
          timestamp: new Date(Date.now() + i * 1000),
          deviceName: `Device ${i}`,
          signalType: "sweep" as const,
          duration: 10,
          sampleRate: 48000,
          outputChannel: "both",
          frequencies: [100],
          rawMagnitudes: [0],
          smoothedMagnitudes: [0],
          rawPhase: [0],
          smoothedPhase: [0],
        });
        ids.push(id);
      }

      // Should only keep the 10 most recent
      const allCaptures = CaptureStorage.getAllCaptures();
      expect(allCaptures.length).toBeLessThanOrEqual(10);

      // Most recent should be kept
      expect(CaptureStorage.getCapture(ids[14])).not.toBeNull();

      // Oldest should be removed
      expect(CaptureStorage.getCapture(ids[0])).toBeNull();
    });
  });

  describe("Data Integrity", () => {
    it("should preserve all fields when saving and loading", () => {
      const original = {
        timestamp: new Date("2024-01-15T10:30:00Z"),
        deviceName: "Precise Test Device",
        signalType: "sweep" as const,
        duration: 12.5,
        sampleRate: 192000,
        outputChannel: "both",
        frequencies: [20, 50, 100, 200, 500, 1000, 2000, 5000, 10000, 20000],
        rawMagnitudes: [
          0, -1.2, -2.4, -3.6, -4.8, -6.0, -7.2, -8.4, -9.6, -10.8,
        ],
        smoothedMagnitudes: [
          0.1, -1.1, -2.3, -3.5, -4.7, -5.9, -7.1, -8.3, -9.5, -10.7,
        ],
        rawPhase: [0, 10, 20, 30, 40, 50, 60, 70, 80, 90],
        smoothedPhase: [
          0.5, 10.5, 20.5, 30.5, 40.5, 50.5, 60.5, 70.5, 80.5, 90.5,
        ],
      };

      const id = CaptureStorage.saveCapture(original);
      const retrieved = CaptureStorage.getCapture(id);

      expect(retrieved).not.toBeNull();
      if (retrieved) {
        expect(retrieved.deviceName).toBe(original.deviceName);
        expect(retrieved.signalType).toBe(original.signalType);
        expect(retrieved.duration).toBe(original.duration);
        expect(retrieved.sampleRate).toBe(original.sampleRate);
        expect(retrieved.outputChannel).toBe(original.outputChannel);
        expect(retrieved.frequencies).toEqual(original.frequencies);
        expect(retrieved.rawMagnitudes).toEqual(original.rawMagnitudes);
        expect(retrieved.smoothedMagnitudes).toEqual(
          original.smoothedMagnitudes,
        );
        expect(retrieved.rawPhase).toEqual(original.rawPhase);
        expect(retrieved.smoothedPhase).toEqual(original.smoothedPhase);
        expect(new Date(retrieved.timestamp).getTime()).toBe(
          original.timestamp.getTime(),
        );
      }
    });

    it("should generate unique IDs for each capture", () => {
      const captureData = {
        timestamp: new Date(),
        deviceName: "ID Test",
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

      const ids = new Set<string>();
      for (let i = 0; i < 10; i++) {
        const id = CaptureStorage.saveCapture(captureData);
        ids.add(id);
      }

      // All IDs should be unique
      expect(ids.size).toBe(10);
    });

    it("should generate meaningful display names", () => {
      const testCases = [
        {
          data: {
            timestamp: new Date("2024-01-15T10:30:00Z"),
            deviceName: "Test Mic",
            signalType: "sweep" as const,
            duration: 10,
            sampleRate: 48000,
            outputChannel: "both",
            frequencies: [100],
            rawMagnitudes: [0],
            smoothedMagnitudes: [0],
            rawPhase: [0],
            smoothedPhase: [0],
          },
          expectedInName: ["Stereo", "Sweep"],
        },
        {
          data: {
            timestamp: new Date("2024-01-15T14:45:30Z"),
            deviceName: "Another Device",
            signalType: "white" as const,
            duration: 15,
            sampleRate: 96000,
            outputChannel: "left",
            frequencies: [200],
            rawMagnitudes: [-3],
            smoothedMagnitudes: [-2.9],
            rawPhase: [45],
            smoothedPhase: [45],
          },
          expectedInName: ["Left", "White"],
        },
      ];

      testCases.forEach((testCase) => {
        const id = CaptureStorage.saveCapture(testCase.data);
        const retrieved = CaptureStorage.getCapture(id);

        expect(retrieved).not.toBeNull();
        expect(retrieved?.name).toBeDefined();
        expect(retrieved?.name.length).toBeGreaterThan(0);

        // Name should include timestamp, channel, and signal type
        testCase.expectedInName.forEach((expectedPart) => {
          expect(retrieved?.name).toContain(expectedPart);
        });
      });
    });
  });

  describe("Non-Regression Tests", () => {
    it("should maintain backwards compatibility with existing storage format", () => {
      // This test ensures that changes to CaptureStorage don't break existing stored data
      const legacyFormat = {
        version: "1.0",
        captures: [
          {
            id: "test-legacy-id",
            name: "Legacy Capture",
            timestamp: new Date("2024-01-01T00:00:00Z"),
            deviceName: "Legacy Device",
            signalType: "sweep",
            duration: 10,
            sampleRate: 48000,
            outputChannel: "both",
            frequencies: [100, 1000],
            rawMagnitudes: [0, -3],
            smoothedMagnitudes: [0.1, -2.9],
            rawPhase: [0, 45],
            smoothedPhase: [0, 45],
          },
        ],
      };

      // Simulate existing data in localStorage
      mockStorage["autoeq_captured_curves"] = JSON.stringify(legacyFormat);

      // Should be able to read legacy data
      const captures = CaptureStorage.getAllCaptures();
      expect(captures.length).toBe(1);
      expect(captures[0].deviceName).toBe("Legacy Device");
    });

    it("should handle missing optional fields gracefully", () => {
      const minimalCapture = {
        timestamp: new Date(),
        deviceName: "Minimal Device",
        signalType: "sweep" as const,
        duration: 10,
        sampleRate: 48000,
        outputChannel: "both",
        frequencies: [100],
        rawMagnitudes: [0],
        smoothedMagnitudes: [0],
        rawPhase: [], // Empty phase data
        smoothedPhase: [],
      };

      const id = CaptureStorage.saveCapture(minimalCapture);
      const retrieved = CaptureStorage.getCapture(id);

      expect(retrieved).not.toBeNull();
      expect(retrieved?.rawPhase).toEqual([]);
      expect(retrieved?.smoothedPhase).toEqual([]);
    });
  });
});
