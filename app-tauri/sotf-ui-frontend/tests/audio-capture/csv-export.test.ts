/**
 * Tests for csv-export module
 * Tests CSV generation, validation, and download trigger
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import {
  CSVExporter,
  type ExportData,
  type CaptureMetadata,
} from "@audio-capture/csv-export";

describe("csv-export", () => {
  const mockMetadata: CaptureMetadata = {
    timestamp: new Date("2025-01-15T10:30:00Z"),
    deviceName: "Test Microphone",
    signalType: "sweep",
    duration: 10,
    sampleRate: 48000,
    outputChannel: "both",
  };

  const mockExportData: ExportData = {
    frequencies: [100, 1000, 10000],
    rawMagnitudes: [0.5, -2.3, -6.1],
    smoothedMagnitudes: [0.4, -2.2, -6.0],
    rawPhase: [5.2, 45.8, 90.3],
    smoothedPhase: [5.0, 45.0, 90.0],
    metadata: mockMetadata,
  };

  describe("validateExportData", () => {
    it("should return empty array for valid data", () => {
      const errors = CSVExporter.validateExportData(mockExportData);
      expect(errors).toEqual([]);
    });

    it("should detect missing frequency data", () => {
      const invalidData = { ...mockExportData, frequencies: [] };
      const errors = CSVExporter.validateExportData(invalidData);
      expect(errors).toContain("No frequency data available");
    });

    it("should detect missing raw magnitude data", () => {
      const invalidData = { ...mockExportData, rawMagnitudes: [] };
      const errors = CSVExporter.validateExportData(invalidData);
      expect(errors).toContain("No raw magnitude data available");
    });

    it("should detect missing smoothed magnitude data", () => {
      const invalidData = { ...mockExportData, smoothedMagnitudes: [] };
      const errors = CSVExporter.validateExportData(invalidData);
      expect(errors).toContain("No smoothed magnitude data available");
    });

    it("should detect missing metadata", () => {
      const invalidData = { ...mockExportData, metadata: undefined };
      const errors = CSVExporter.validateExportData(invalidData as any);
      expect(errors).toContain("No capture metadata available");
    });

    it("should detect data length mismatch", () => {
      const invalidData = {
        ...mockExportData,
        frequencies: [100, 1000],
        rawMagnitudes: [0.5, -2.3, -6.1],
        smoothedMagnitudes: [0.4, -2.2, -6.0],
      };
      const errors = CSVExporter.validateExportData(invalidData);
      expect(errors.length).toBeGreaterThan(0);
      expect(errors[0]).toContain("Data length mismatch");
    });

    it("should detect multiple errors", () => {
      const invalidData = {
        frequencies: [],
        rawMagnitudes: [],
        smoothedMagnitudes: [],
      };
      const errors = CSVExporter.validateExportData(invalidData as any);
      expect(errors.length).toBeGreaterThanOrEqual(3);
    });
  });

  describe("generatePreview", () => {
    it("should generate preview with header and data rows", () => {
      const preview = CSVExporter.generatePreview(mockExportData, 5);

      expect(preview).toContain("# AutoEQ Audio Capture Data");
      expect(preview).toContain("# Capture Date: 2025-01-15");
      expect(preview).toContain("# Device: Test Microphone");
      expect(preview).toContain(
        "Frequency(Hz),Raw_SPL(dB),Smoothed_SPL(dB),Raw_Phase(deg),Smoothed_Phase(deg)",
      );
      expect(preview).toContain("100.00,0.500,0.400,5.2,5.0");
    });

    it("should include phase columns when phase data is available", () => {
      const preview = CSVExporter.generatePreview(mockExportData);

      expect(preview).toContain("Raw_Phase(deg)");
      expect(preview).toContain("Smoothed_Phase(deg)");
    });

    it("should exclude phase columns when phase data is missing", () => {
      const dataWithoutPhase = {
        ...mockExportData,
        rawPhase: undefined,
        smoothedPhase: undefined,
      };
      const preview = CSVExporter.generatePreview(dataWithoutPhase);

      expect(preview).not.toContain("Raw_Phase");
      expect(preview).not.toContain("Smoothed_Phase");
      expect(preview).toContain("Frequency(Hz),Raw_SPL(dB),Smoothed_SPL(dB)");
    });

    it("should truncate preview when maxLines is specified", () => {
      const preview = CSVExporter.generatePreview(mockExportData, 2);
      const lines = preview.split("\n");

      // Should have header comments (9 lines) + column header (1) + 2 data rows + truncation message
      expect(lines.length).toBeLessThanOrEqual(13);
      expect(preview).toContain("... and");
    });
  });

  describe("toOptimizationFormat", () => {
    it("should convert to optimization format with smoothed data", () => {
      const format = CSVExporter.toOptimizationFormat(mockExportData);

      expect(format.frequencies).toEqual(mockExportData.frequencies);
      expect(format.magnitudes).toEqual(mockExportData.smoothedMagnitudes);
    });

    it("should create copies of arrays", () => {
      const format = CSVExporter.toOptimizationFormat(mockExportData);

      expect(format.frequencies).not.toBe(mockExportData.frequencies);
      expect(format.magnitudes).not.toBe(mockExportData.smoothedMagnitudes);
    });
  });

  describe("exportToCSV", () => {
    let linkElement: any;

    beforeEach(() => {
      // Create a mock link element that we can track
      linkElement = {
        setAttribute: vi.fn(),
        click: vi.fn(),
        href: "",
        download: "",
        style: { display: "" },
      };

      // Mock DOM methods
      global.document.createElement = vi.fn((tag: string) => {
        if (tag === "a") {
          return linkElement;
        }
        return {} as any;
      });

      global.document.body.appendChild = vi.fn();
      global.document.body.removeChild = vi.fn();

      global.URL.createObjectURL = vi.fn(() => "blob:mock-url");
      global.URL.revokeObjectURL = vi.fn();

      global.Blob = vi.fn(function (content: any[], options: any) {
        return {
          content,
          options,
        };
      }) as any;
    });

    it("should trigger download with correct filename", () => {
      CSVExporter.exportToCSV(mockExportData);

      const createElement = global.document.createElement as any;
      expect(createElement).toHaveBeenCalledWith("a");

      // Check the download attribute was set correctly
      expect(linkElement.download).toContain("2025-01-15");
      expect(linkElement.download).toContain("both");
      expect(linkElement.download).toContain("sweep");
      expect(linkElement.download).toMatch(/\.csv$/);
    });

    it("should create blob with correct content type", () => {
      CSVExporter.exportToCSV(mockExportData);

      expect(global.Blob).toHaveBeenCalledWith(
        expect.any(Array),
        expect.objectContaining({ type: "text/csv;charset=utf-8;" }),
      );
    });

    it("should trigger click on link element", async () => {
      // Mock setTimeout to execute immediately
      const originalSetTimeout = global.setTimeout;
      global.setTimeout = ((fn: any) => {
        fn();
        return 0 as any;
      }) as any;

      try {
        CSVExporter.exportToCSV(mockExportData);

        expect(linkElement.click).toHaveBeenCalled();
      } finally {
        global.setTimeout = originalSetTimeout;
      }
    });

    it("should clean up after download", () => {
      // Mock setTimeout to execute immediately
      const originalSetTimeout = global.setTimeout;
      global.setTimeout = ((fn: any) => {
        fn();
        return 0 as any;
      }) as any;

      try {
        CSVExporter.exportToCSV(mockExportData);

        // Verify appendChild is called immediately
        expect(global.document.body.appendChild).toHaveBeenCalled();

        // With immediate setTimeout, cleanup should have happened
        expect(global.document.body.removeChild).toHaveBeenCalled();
        expect(global.URL.revokeObjectURL).toHaveBeenCalled();
      } finally {
        global.setTimeout = originalSetTimeout;
      }
    });
  });

  describe("CSV content generation", () => {
    it("should format numbers correctly", () => {
      const data: ExportData = {
        frequencies: [1234.567],
        rawMagnitudes: [1.234567],
        smoothedMagnitudes: [-5.678901],
        metadata: mockMetadata,
      };

      const preview = CSVExporter.generatePreview(data);

      // Frequencies should have 2 decimal places
      expect(preview).toContain("1234.57");
      // Magnitudes should have 3 decimal places
      expect(preview).toContain("1.235");
      expect(preview).toContain("-5.679");
    });

    it("should handle missing phase values gracefully", () => {
      const dataWithPartialPhase: ExportData = {
        ...mockExportData,
        rawPhase: [1.0, undefined as any, 3.0],
        smoothedPhase: [1.0, 2.0, undefined as any],
      };

      const preview = CSVExporter.generatePreview(dataWithPartialPhase);

      // Should substitute 0.0 for undefined phase values
      expect(preview).toContain("0.0");
    });

    it("should include all metadata in header comments", () => {
      const preview = CSVExporter.generatePreview(mockExportData);

      expect(preview).toContain("# Device: Test Microphone");
      expect(preview).toContain("# Signal Type: sweep");
      expect(preview).toContain("# Duration: 10s");
      expect(preview).toContain("# Sample Rate: 48000Hz");
      expect(preview).toContain("# Output Channel: both");
      expect(preview).toContain("# Generated by AutoEQ App");
    });

    it("should use ISO format for timestamp", () => {
      const preview = CSVExporter.generatePreview(mockExportData);

      expect(preview).toContain("2025-01-15T10:30:00.000Z");
    });
  });
});
