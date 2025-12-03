// Tests for API Manager functionality
import { describe, test, expect, beforeEach, vi, afterEach } from "vitest";

// Mock the @tauri-apps/api/core module
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

// Mock the @tauri-apps/plugin-dialog module
vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
}));

import { APIManager } from "../modules/api-manager";
import { OPTIMIZATION_DEFAULTS } from "../modules/optimization-constants";
import { invoke } from "@tauri-apps/api/core";

// Get the mocked invoke function
const mockTauriInvoke = vi.mocked(invoke);

// Also mock the global Tauri object for completeness
(globalThis as any).window = {
  __TAURI__: {
    core: {
      invoke: mockTauriInvoke,
    },
  },
};

// Mock DOM elements
const createMockElement = (tagName: string = "div", type?: string) => {
  const element = {
    tagName: tagName.toUpperCase(),
    type: type || "",
    id: "",
    className: "",
    classList: {
      add: vi.fn(),
      remove: vi.fn(),
      contains: vi.fn(() => false),
    },
    style: {},
    value: "",
    innerHTML: "",
    textContent: "",
    disabled: false,
    appendChild: vi.fn(),
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    dispatchEvent: vi.fn(),
    remove: vi.fn(),
    parentElement: null,
    closest: vi.fn(() => null),
    querySelector: vi.fn(() => null),
    querySelectorAll: vi.fn(() => []),
    getBoundingClientRect: vi.fn(() => ({
      top: 0,
      left: 0,
      bottom: 30,
      right: 200,
      width: 200,
      height: 30,
    })),
    focus: vi.fn(),
    removeAttribute: vi.fn(),
    setAttribute: vi.fn(),
    getAttribute: vi.fn(() => null),
    hasAttribute: vi.fn(() => false),
    cloneNode: vi.fn(() => createMockElement(tagName, type)),
    replaceChild: vi.fn(),
    offsetHeight: 0,
    offsetWidth: 0,
    options: [] as any[],
    selectedIndex: 0,
  };

  if (tagName === "select") {
    element.options = [];
    element.selectedIndex = 0;
    // Mock methods for options manipulation
    (element as any).add = vi.fn();
    (element as any).remove = vi.fn();
  }

  return element;
};

// Mock document.getElementById
const mockElements: { [key: string]: any } = {};
const originalGetElementById = document.getElementById;

beforeEach(() => {
  vi.clearAllMocks();

  // Reset mock elements
  Object.keys(mockElements).forEach((key) => delete mockElements[key]);

  // Create common mock elements
  mockElements["speaker"] = createMockElement("input", "text");
  mockElements["version"] = createMockElement("select");
  mockElements["measurement"] = createMockElement("select");
  mockElements["curve-path"] = createMockElement("input", "text");
  mockElements["target-path"] = createMockElement("input", "text");
  mockElements["demo-audio-select"] = createMockElement("select");

  // Mock document.getElementById
  document.getElementById = vi.fn((id: string) => mockElements[id] || null);

  // Mock other DOM methods
  document.createElement = vi.fn((tagName: string) =>
    createMockElement(tagName),
  ) as any;
  document.querySelector = vi.fn(() => null) as any;
  document.querySelectorAll = vi.fn(() => []) as any;
  document.addEventListener = vi.fn() as any;

  // Mock window properties
  (window as any).scrollY = 0;
  (window as any).scrollX = 0;

  // Mock requestAnimationFrame
  global.requestAnimationFrame = vi.fn((callback) => {
    setTimeout(callback, 16);
    return 1;
  });

  Object.defineProperty(window, "matchMedia", {
    writable: true,
    value: vi.fn().mockImplementation((query) => ({
      matches: false,
      media: query,
      onchange: null,
      addListener: vi.fn(),
      removeListener: vi.fn(),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      dispatchEvent: vi.fn(),
    })),
  });
});

afterEach(() => {
  document.getElementById = originalGetElementById;
  vi.restoreAllMocks();
});

describe("APIManager", () => {
  let apiManager: APIManager;

  beforeEach(() => {
    apiManager = new APIManager();
  });

  describe("Constructor and Initialization", () => {
    test("should initialize with empty speakers array on API failure", async () => {
      mockTauriInvoke.mockRejectedValue(new Error("API not available"));

      const newApiManager = new APIManager();

      // Wait for async initialization
      await new Promise((resolve) => setTimeout(resolve, 10));

      expect(newApiManager.getSpeakers()).toEqual([]);
    });

    test("should load speakers from API on successful call", async () => {
      const mockSpeakers = ["Speaker A", "Speaker B", "Speaker C"];
      mockTauriInvoke.mockResolvedValue(mockSpeakers);

      const newApiManager = new APIManager();

      // Wait for async initialization
      await new Promise((resolve) => setTimeout(resolve, 10));

      expect(mockTauriInvoke).toHaveBeenCalledWith("get_speakers");
      expect(newApiManager.getSpeakers()).toEqual(mockSpeakers);
    });
  });

  describe("loadSpeakerVersions", () => {
    test("should load versions from API successfully", async () => {
      const mockVersions = ["v1.0", "v2.0", "latest"];
      mockTauriInvoke.mockResolvedValue(mockVersions);

      const versions = await apiManager.loadSpeakerVersions("Test Speaker");

      expect(mockTauriInvoke).toHaveBeenCalledWith("get_speaker_versions", {
        speaker: "Test Speaker",
      });
      expect(versions).toEqual(mockVersions);
    });

    test("should handle object response format with versions property", async () => {
      const mockResponse = { versions: ["v1.0", "v2.0"] };
      mockTauriInvoke.mockResolvedValue(mockResponse);

      const versions = await apiManager.loadSpeakerVersions("Test Speaker");

      expect(versions).toEqual(["v1.0", "v2.0"]);
    });

    test("should handle invalid response format gracefully", async () => {
      mockTauriInvoke.mockResolvedValue("invalid response");

      const versions = await apiManager.loadSpeakerVersions("Test Speaker");

      expect(versions).toEqual([]);
    });

    test("should handle API errors gracefully", async () => {
      mockTauriInvoke.mockRejectedValue(new Error("API Error"));

      const versions = await apiManager.loadSpeakerVersions("Test Speaker");

      expect(versions).toEqual([]);
    });

    test("should cache loaded versions", async () => {
      const mockVersions = ["v1.0", "v2.0"];
      mockTauriInvoke.mockResolvedValue(mockVersions);

      await apiManager.loadSpeakerVersions("Test Speaker");
      const speakerData = apiManager.getSpeakerData("Test Speaker");

      expect(speakerData?.versions).toEqual(mockVersions);
    });
  });

  describe("loadSpeakerMeasurements", () => {
    test("should load measurements from API successfully", async () => {
      const mockMeasurements = ["On Axis", "Listening Window"];
      mockTauriInvoke.mockResolvedValue(mockMeasurements);

      const measurements = await apiManager.loadSpeakerMeasurements(
        "Test Speaker",
        "v1.0",
      );

      expect(mockTauriInvoke).toHaveBeenCalledWith("get_speaker_measurements", {
        speaker: "Test Speaker",
        version: "v1.0",
      });
      expect(measurements).toEqual(mockMeasurements);
    });

    test("should handle object response format with measurements property", async () => {
      const mockResponse = { measurements: ["On Axis", "Off Axis"] };
      mockTauriInvoke.mockResolvedValue(mockResponse);

      const measurements = await apiManager.loadSpeakerMeasurements(
        "Test Speaker",
        "v1.0",
      );

      expect(measurements).toEqual(["On Axis", "Off Axis"]);
    });

    test("should handle API errors with empty measurements", async () => {
      mockTauriInvoke.mockRejectedValue(new Error("API Error"));

      const measurements = await apiManager.loadSpeakerMeasurements(
        "Test Speaker",
        "v1.0",
      );

      expect(measurements).toEqual([]);
    });
  });

  describe("handleSpeakerChange", () => {
    test("should clear and disable dependent dropdowns when no speaker selected", async () => {
      const versionSelect = mockElements["version"];
      const measurementSelect = mockElements["measurement"];

      await apiManager.handleSpeakerChange("");

      expect(versionSelect.innerHTML).toBe(
        '<option value="">Select a version...</option>',
      );
      expect(measurementSelect.innerHTML).toBe(
        '<option value="">Select a measurement...</option>',
      );
      expect(versionSelect.disabled).toBe(true);
      expect(measurementSelect.disabled).toBe(true);
    });

    test("should load versions and enable version dropdown when speaker selected", async () => {
      const mockVersions = ["v1.0", "v2.0"];
      mockTauriInvoke.mockResolvedValue(mockVersions);
      const versionSelect = mockElements["version"];

      await apiManager.handleSpeakerChange("Test Speaker");

      expect(mockTauriInvoke).toHaveBeenCalledWith("get_speaker_versions", {
        speaker: "Test Speaker",
      });
      expect(versionSelect.disabled).toBe(false);
      expect(versionSelect.appendChild).toHaveBeenCalled();
    });

    test("should set first version as selected when versions are loaded", async () => {
      const mockVersions = ["v1.0", "v2.0", "v3.0"];
      const mockMeasurements = ["On Axis", "Listening Window"];
      mockTauriInvoke
        .mockResolvedValueOnce(mockVersions) // for get_speaker_versions
        .mockResolvedValueOnce(mockMeasurements); // for get_speaker_measurements

      const versionSelect = mockElements["version"];
      const measurementSelect = mockElements["measurement"];
      const lossSelect = (mockElements["loss"] = createMockElement("select"));

      await apiManager.handleSpeakerChange("Test Speaker");

      // Should be called multiple times - once for each version
      expect(versionSelect.appendChild).toHaveBeenCalledTimes(
        mockVersions.length,
      );
      // Should automatically select the first version
      expect(versionSelect.value).toBe("v1.0");
      expect(apiManager.getSelectedVersion()).toBe("v1.0");
      // Should automatically select the first measurement
      expect(measurementSelect.value).toBe("On Axis");
      // Should set loss function to speaker-flat
      expect(lossSelect.value).toBe("speaker-flat");
    });

    test("should handle API errors by keeping version dropdown disabled", async () => {
      mockTauriInvoke.mockRejectedValue(new Error("API Error"));
      const versionSelect = mockElements["version"];

      await apiManager.handleSpeakerChange("Test Speaker");

      expect(versionSelect.disabled).toBe(true);
    });

    test("should auto-select first version when changing speakers", async () => {
      const mockMeasurements = ["On Axis"];
      mockTauriInvoke
        .mockResolvedValueOnce(["v1.0"]) // Speaker 1 versions
        .mockResolvedValueOnce(mockMeasurements) // Speaker 1 measurements
        .mockResolvedValueOnce(["v2.0"]) // Speaker 2 versions
        .mockResolvedValueOnce(mockMeasurements); // Speaker 2 measurements

      await apiManager.handleSpeakerChange("Speaker 1");
      expect(apiManager.getSelectedVersion()).toBe("v1.0");

      await apiManager.handleSpeakerChange("Speaker 2");
      expect(apiManager.getSelectedVersion()).toBe("v2.0");
    });
  });

  describe("handleVersionChange", () => {
    test("should clear and disable measurement dropdown when no version selected", async () => {
      const measurementSelect = mockElements["measurement"];

      await apiManager.handleVersionChange("");

      expect(measurementSelect.innerHTML).toBe(
        '<option value="">Select a measurement...</option>',
      );
      expect(measurementSelect.disabled).toBe(true);
    });

    test("should load measurements and enable dropdown when version selected", async () => {
      const mockMeasurements = ["On Axis", "Listening Window"];
      mockTauriInvoke.mockResolvedValue(mockMeasurements);
      const measurementSelect = mockElements["measurement"];

      // First set a speaker
      apiManager["selectedSpeaker"] = "Test Speaker";

      await apiManager.handleVersionChange("v1.0");

      expect(mockTauriInvoke).toHaveBeenCalledWith("get_speaker_measurements", {
        speaker: "Test Speaker",
        version: "v1.0",
      });
      expect(measurementSelect.disabled).toBe(false);
      expect(measurementSelect.appendChild).toHaveBeenCalled();
      // Should automatically select the first measurement
      expect(measurementSelect.value).toBe("On Axis");
    });

    test("should handle missing speaker gracefully", async () => {
      // Clear any previous calls from constructor
      mockTauriInvoke.mockClear();

      const measurementSelect = mockElements["measurement"];

      await apiManager.handleVersionChange("v1.0");

      expect(measurementSelect.disabled).toBe(true);
      expect(mockTauriInvoke).not.toHaveBeenCalled();
    });
  });

  describe("validateOptimizationParams", () => {
    test("should validate speaker selection parameters", () => {
      const formData = new FormData();
      formData.set("input_source", "api");
      formData.set("speaker", "Test Speaker");
      formData.set("version", "v1.0");
      formData.set("measurement", "On Axis");

      const result = apiManager.validateOptimizationParams(formData);

      expect(result.isValid).toBe(true);
      expect(result.errors).toHaveLength(0);
    });

    test("should detect missing speaker selection parameters", () => {
      const formData = new FormData();
      formData.set("input_source", "speaker");
      // Missing speaker, version, measurement

      const result = apiManager.validateOptimizationParams(formData);

      expect(result.isValid).toBe(false);
      expect(result.errors).toContain("Speaker selection is required");
      expect(result.errors).toContain("Version selection is required");
      expect(result.errors).toContain("Measurement selection is required");
    });

    test("should validate file selection parameters", () => {
      const formData = new FormData();
      formData.set("input_source", "file");
      formData.set("curve_path", "/path/to/curve.csv");
      formData.set("target_path", "/path/to/target.csv");

      const result = apiManager.validateOptimizationParams(formData);

      expect(result.isValid).toBe(true);
      expect(result.errors).toHaveLength(0);
    });

    test("should detect missing file paths", () => {
      const formData = new FormData();
      formData.set("input_source", "file");
      // Missing curve_path and target_path

      const result = apiManager.validateOptimizationParams(formData);

      expect(result.isValid).toBe(false);
      expect(result.errors).toContain("Curve file is required");
      expect(result.errors).toContain("Target file is required");
    });
  });

  describe("Autocomplete functionality", () => {
    test("should setup autocomplete event listeners", () => {
      const speakerInput = mockElements["speaker"];

      apiManager.setupAutocomplete();

      expect(speakerInput.addEventListener).toHaveBeenCalledWith(
        "input",
        expect.any(Function),
      );
      expect(speakerInput.addEventListener).toHaveBeenCalledWith(
        "blur",
        expect.any(Function),
      );
    });

    test("should return autocomplete data", () => {
      apiManager["autocompleteData"] = ["Speaker A", "Speaker B"];

      const data = apiManager.getAutocompleteData();

      expect(data).toEqual(["Speaker A", "Speaker B"]);
      expect(data).not.toBe(apiManager["autocompleteData"]); // Should return a copy
    });
  });

  describe("Getters", () => {
    test("should return speakers array copy", () => {
      apiManager["speakers"] = ["Speaker A", "Speaker B"];

      const speakers = apiManager.getSpeakers();

      expect(speakers).toEqual(["Speaker A", "Speaker B"]);
      expect(speakers).not.toBe(apiManager["speakers"]); // Should return a copy
    });

    test("should return selected speaker", () => {
      apiManager["selectedSpeaker"] = "Test Speaker";

      expect(apiManager.getSelectedSpeaker()).toBe("Test Speaker");
    });

    test("should return selected version", () => {
      apiManager["selectedVersion"] = "v1.0";

      expect(apiManager.getSelectedVersion()).toBe("v1.0");
    });

    test("should return speaker data", () => {
      const testData = {
        name: "Test Speaker",
        versions: ["v1.0"],
        measurements: { "v1.0": ["On Axis"] },
      };
      apiManager["speakerData"]["Test Speaker"] = testData;

      const data = apiManager.getSpeakerData("Test Speaker");

      expect(data).toEqual(testData);
    });

    test("should return null for non-existent speaker data", () => {
      const data = apiManager.getSpeakerData("Non-existent Speaker");

      expect(data).toBeNull();
    });
  });

  describe("Optimization Constants Integration", () => {
    test("should use constants for default values", () => {
      // Test that our constants are properly defined
      expect(OPTIMIZATION_DEFAULTS.num_filters).toBe(5);
      expect(OPTIMIZATION_DEFAULTS.sample_rate).toBe(48000);
      expect(OPTIMIZATION_DEFAULTS.min_db).toBe(1.0);
      expect(OPTIMIZATION_DEFAULTS.max_db).toBe(3.0);
      expect(OPTIMIZATION_DEFAULTS.algo).toBe("autoeq:de");
      expect(OPTIMIZATION_DEFAULTS.loss).toBe("speaker-flat");
      expect(OPTIMIZATION_DEFAULTS.input_source).toBe("file");
    });

    test("should validate parameters with default fallbacks", () => {
      const formData = new FormData();
      formData.set("input_source", "speaker");
      formData.set("speaker", "Test Speaker");
      formData.set("version", "v1.0");
      formData.set("measurement", "On Axis");
      // Not setting other parameters to test defaults

      const result = apiManager.validateOptimizationParams(formData);

      expect(result.isValid).toBe(true);
      expect(result.errors).toHaveLength(0);
    });
  });
});
