// Test setup file for Vitest
// This file is run before each test file

import { vi, beforeAll, afterAll } from "vitest";

// Mock Plotly.js
vi.mock("plotly.js-dist-min", () => ({
  newPlot: vi.fn().mockResolvedValue(undefined),
  Plots: {
    resize: vi.fn(),
  },
  purge: vi.fn(),
}));

// Mock Tauri API modules
vi.mock("@tauri-apps/api/tauri", () => ({
  invoke: vi.fn().mockResolvedValue({}),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
  once: vi.fn().mockResolvedValue(() => {}),
  emit: vi.fn().mockResolvedValue(undefined),
}));

// Mock Tauri API on window for backward compatibility
Object.defineProperty(window, "__TAURI__", {
  value: {
    core: {
      invoke: vi.fn(),
    },
    event: {
      listen: vi.fn().mockResolvedValue(() => {}),
    },
  },
  writable: true,
});

// Mock DOM methods that might not be available in jsdom
Object.defineProperty(HTMLElement.prototype, "offsetWidth", {
  configurable: true,
  value: 800,
});

Object.defineProperty(HTMLElement.prototype, "offsetHeight", {
  configurable: true,
  value: 600,
});

// Mock getComputedStyle
Object.defineProperty(window, "getComputedStyle", {
  value: () => ({
    getPropertyValue: (prop: string) => {
      if (prop === "--text-primary") return "#333333";
      if (prop === "--text-secondary") return "#666666";
      if (prop === "--bg-primary") return "#ffffff";
      if (prop === "--bg-secondary") return "#f5f5f5";
      return "";
    },
  }),
});

// Mock performance.now for performance tests
Object.defineProperty(window, "performance", {
  value: {
    now: vi.fn(() => Date.now()),
  },
});

// Global test utilities
(globalThis as any).createMockElement = () => ({
  innerHTML: "",
  style: { display: "block", padding: "0" },
  classList: { add: vi.fn(), remove: vi.fn() },
  offsetWidth: 800,
  offsetHeight: 600,
  closest: vi.fn().mockReturnValue({
    style: { display: "block" },
  }),
});

// Console error suppression for expected errors in tests
const originalError = console.error;
beforeAll(() => {
  console.error = (...args: any[]) => {
    if (
      typeof args[0] === "string" &&
      args[0].includes("Warning: ReactDOM.render is deprecated")
    ) {
      return;
    }
    originalError.call(console, ...args);
  };
});

afterAll(() => {
  console.error = originalError;
});
