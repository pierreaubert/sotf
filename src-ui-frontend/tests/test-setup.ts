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

// Mock DragEvent for drag-and-drop tests
if (typeof DragEvent === "undefined") {
  global.DragEvent = class DragEvent extends Event {
    dataTransfer: any;
    constructor(type: string, eventInitDict?: any) {
      super(type, eventInitDict);
      this.dataTransfer = eventInitDict?.dataTransfer || null;
    }
  } as any;
}

// Mock gradient object for canvas
const mockGradient = {
  addColorStop: vi.fn(),
};

// Mock HTMLCanvasElement.getContext for canvas tests
const mockCanvasContext = {
  fillStyle: "",
  strokeStyle: "",
  lineWidth: 1,
  font: "10px sans-serif",
  textAlign: "left" as CanvasTextAlign,
  textBaseline: "alphabetic" as CanvasTextBaseline,
  fillRect: vi.fn(),
  strokeRect: vi.fn(),
  clearRect: vi.fn(),
  fillText: vi.fn(),
  strokeText: vi.fn(),
  measureText: vi.fn(() => ({ width: 0 })),
  beginPath: vi.fn(),
  closePath: vi.fn(),
  moveTo: vi.fn(),
  lineTo: vi.fn(),
  arc: vi.fn(),
  arcTo: vi.fn(),
  rect: vi.fn(),
  stroke: vi.fn(),
  fill: vi.fn(),
  save: vi.fn(),
  restore: vi.fn(),
  scale: vi.fn(),
  rotate: vi.fn(),
  translate: vi.fn(),
  transform: vi.fn(),
  setTransform: vi.fn(),
  resetTransform: vi.fn(),
  drawImage: vi.fn(),
  createImageData: vi.fn(),
  getImageData: vi.fn(),
  putImageData: vi.fn(),
  createLinearGradient: vi.fn(() => mockGradient),
  createRadialGradient: vi.fn(() => mockGradient),
  createPattern: vi.fn(),
  setLineDash: vi.fn(),
  getLineDash: vi.fn(() => []),
  canvas: null as any,
};

HTMLCanvasElement.prototype.getContext = vi.fn(function (
  this: HTMLCanvasElement,
  contextType: string,
) {
  if (contextType === "2d") {
    mockCanvasContext.canvas = this;
    return mockCanvasContext as any;
  }
  return null;
}) as any;

// Mock getBoundingClientRect for canvas elements
HTMLCanvasElement.prototype.getBoundingClientRect = vi.fn(function (
  this: HTMLCanvasElement,
) {
  return {
    width: this.width || 800,
    height: this.height || 600,
    top: 0,
    left: 0,
    right: this.width || 800,
    bottom: this.height || 600,
    x: 0,
    y: 0,
    toJSON: () => ({}),
  } as DOMRect;
}) as any;

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
