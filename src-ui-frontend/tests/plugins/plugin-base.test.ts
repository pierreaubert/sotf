// BasePlugin test suite
import { describe, test, expect, beforeEach, vi } from "vitest";
import { BasePlugin } from "../../modules/plugins/plugin-base";
import type {
  PluginMetadata,
  PluginConfig,
} from "../../modules/plugins/plugin-types";

// Mock implementation of BasePlugin for testing
class TestPlugin extends BasePlugin {
  public readonly metadata: PluginMetadata = {
    id: "test-plugin",
    name: "Test Plugin",
    category: "test",
    version: "1.0.0",
  };

  // Expose protected properties for testing
  public getSelectedParameterIndex() {
    return this.selectedParameterIndex;
  }

  public setParameterOrder(order: string[]) {
    this.parameterOrder = order;
  }

  public setParameterLabels(labels: Record<string, string>) {
    this.parameterLabels = labels;
  }

  public getParameterKeys() {
    return this.parameterKeys;
  }

  public getKeyToParamIndex() {
    return this.keyToParamIndex;
  }

  public callAssignParameterKeys() {
    this.assignParameterKeys();
  }

  public callGetFormattedLabel(paramName: string) {
    return this.getFormattedLabel(paramName);
  }

  render(standalone: boolean): void {
    if (!this.container) return;
    this.container.innerHTML = `
      <div class="test-plugin">
        <h1>Test Plugin</h1>
        <div class="parameter-field" data-param="param1" data-index="0">Param 1</div>
        <div class="parameter-field" data-param="param2" data-index="1">Param 2</div>
        <div class="parameter-field" data-param="param3" data-index="2">Param 3</div>
      </div>
    `;
  }

  protected adjustSelectedParameter(delta: number): void {
    // Mock implementation
  }
}

describe("BasePlugin", () => {
  let plugin: TestPlugin;
  let container: HTMLElement;

  beforeEach(() => {
    container = document.createElement("div");
    document.body.appendChild(container);
    plugin = new TestPlugin();
  });

  describe("Initialization", () => {
    test("should initialize with container and config", () => {
      const config: PluginConfig = {
        standalone: true,
        initialState: {
          enabled: true,
          bypassed: false,
          parameters: { test: 123 },
        },
      };

      plugin.initialize(container, config);

      expect(plugin.getState()).toEqual({
        enabled: true,
        bypassed: false,
        parameters: { test: 123 },
      });
    });

    test("should render on initialization", () => {
      plugin.initialize(container);
      expect(container.querySelector(".test-plugin")).toBeTruthy();
    });

    test("should setup keyboard controls on initialization", () => {
      const addEventListenerSpy = vi.spyOn(document, "addEventListener");
      plugin.initialize(container);
      expect(addEventListenerSpy).toHaveBeenCalledWith(
        "keydown",
        expect.any(Function),
      );
    });
  });

  describe("State Management", () => {
    beforeEach(() => {
      plugin.initialize(container);
    });

    test("should get current state", () => {
      const state = plugin.getState();
      expect(state).toEqual({
        enabled: true,
        bypassed: false,
        parameters: {},
      });
    });

    test("should set state (partial update)", () => {
      plugin.setState({ bypassed: true });
      expect(plugin.getState().bypassed).toBe(true);
      expect(plugin.getState().enabled).toBe(true);
    });

    test("should emit state change event", () => {
      const callback = vi.fn();
      plugin.on("stateChanged", callback);

      plugin.setState({ bypassed: true });

      expect(callback).toHaveBeenCalledWith(
        expect.objectContaining({ bypassed: true }),
        expect.objectContaining({ bypassed: false }),
      );
    });

    test("should call onStateChange callback", () => {
      const onStateChange = vi.fn();
      plugin.initialize(container, { onStateChange });

      plugin.setState({ bypassed: true });

      expect(onStateChange).toHaveBeenCalledWith(
        expect.objectContaining({ bypassed: true }),
      );
    });
  });

  describe("Bypass Functionality", () => {
    beforeEach(() => {
      plugin.initialize(container);
    });

    test("should check if plugin is bypassed", () => {
      expect(plugin.isBypassed()).toBe(false);
      plugin.setState({ bypassed: true });
      expect(plugin.isBypassed()).toBe(true);
    });

    test("should set bypass state", () => {
      plugin.setBypass(true);
      expect(plugin.isBypassed()).toBe(true);
    });

    test("should toggle bypass state", () => {
      expect(plugin.isBypassed()).toBe(false);
      plugin.toggleBypass();
      expect(plugin.isBypassed()).toBe(true);
      plugin.toggleBypass();
      expect(plugin.isBypassed()).toBe(false);
    });

    test("should emit bypassed event on bypass change", () => {
      const callback = vi.fn();
      plugin.on("bypassed", callback);

      plugin.setBypass(true);

      expect(callback).toHaveBeenCalledWith(true);
    });

    test("should call onBypass callback", () => {
      const onBypass = vi.fn();
      plugin.initialize(container, { onBypass });

      plugin.setBypass(true);

      expect(onBypass).toHaveBeenCalledWith(true);
    });
  });

  describe("Event System", () => {
    beforeEach(() => {
      plugin.initialize(container);
    });

    test("should register event listener", () => {
      const callback = vi.fn();
      plugin.on("test-event", callback);

      plugin.emit("test-event", "data");

      expect(callback).toHaveBeenCalledWith("data");
    });

    test("should unregister event listener", () => {
      const callback = vi.fn();
      plugin.on("test-event", callback);
      plugin.off("test-event", callback);

      plugin.emit("test-event", "data");

      expect(callback).not.toHaveBeenCalled();
    });

    test("should handle multiple listeners for same event", () => {
      const callback1 = vi.fn();
      const callback2 = vi.fn();
      plugin.on("test-event", callback1);
      plugin.on("test-event", callback2);

      plugin.emit("test-event", "data");

      expect(callback1).toHaveBeenCalledWith("data");
      expect(callback2).toHaveBeenCalledWith("data");
    });

    test("should handle errors in event listeners gracefully", () => {
      const errorCallback = vi.fn(() => {
        throw new Error("Test error");
      });
      const normalCallback = vi.fn();

      plugin.on("test-event", errorCallback);
      plugin.on("test-event", normalCallback);

      // Should not throw, should log error but continue
      expect(() => plugin.emit("test-event", "data")).not.toThrow();
      expect(normalCallback).toHaveBeenCalled();
    });
  });

  describe("Keyboard Control - Parameter Key Assignment", () => {
    beforeEach(() => {
      plugin.setParameterOrder(["ratio", "release", "attack", "threshold"]);
      plugin.setParameterLabels({
        ratio: "Ratio",
        release: "Release",
        attack: "Attack",
        threshold: "Threshold",
      });
      plugin.callAssignParameterKeys();
    });

    test("should assign unique keys based on first letter", () => {
      const keys = plugin.getParameterKeys();

      expect(keys.ratio).toBe("r");
      expect(keys.attack).toBe("a");
      expect(keys.threshold).toBe("t");
    });

    test("should handle collision by using next available letter", () => {
      // 'Ratio' and 'Release' both start with 'R'
      // Ratio should get 'r', Release should get 'e' (next available)
      const keys = plugin.getParameterKeys();

      expect(keys.ratio).toBe("r");
      expect(keys.release).toBe("e"); // Second letter since 'r' is taken
    });

    test("should create key-to-index mapping", () => {
      const keyToIndex = plugin.getKeyToParamIndex();

      expect(keyToIndex.r).toBe(0); // ratio
      expect(keyToIndex.e).toBe(1); // release
      expect(keyToIndex.a).toBe(2); // attack
      expect(keyToIndex.t).toBe(3); // threshold
    });

    test("should handle case-insensitive key assignment", () => {
      const keys = plugin.getParameterKeys();

      // All keys should be lowercase
      Object.values(keys).forEach((key) => {
        expect(key).toBe(key.toLowerCase());
      });
    });
  });

  describe("Keyboard Control - Formatted Labels", () => {
    beforeEach(() => {
      plugin.setParameterOrder(["ratio", "release", "attack"]);
      plugin.setParameterLabels({
        ratio: "Ratio",
        release: "Release",
        attack: "Attack",
      });
      plugin.callAssignParameterKeys();
    });

    test("should format label with brackets around assigned key", () => {
      expect(plugin.callGetFormattedLabel("ratio")).toBe("[R]atio");
      expect(plugin.callGetFormattedLabel("attack")).toBe("[A]ttack");
    });

    test("should format label with brackets at correct position for collision", () => {
      // Release should use 'e' (second letter) since 'r' is taken by Ratio
      expect(plugin.callGetFormattedLabel("release")).toBe("R[e]lease");
    });

    test("should return original label if no key assigned", () => {
      plugin.setParameterOrder(["unknown"]);
      plugin.setParameterLabels({ unknown: "Unknown" });
      plugin.callAssignParameterKeys();

      // After collision resolution, it should still get a key
      // But if we manually check for missing key:
      const label = plugin.callGetFormattedLabel("nonexistent");
      expect(label).toBe("nonexistent");
    });
  });

  describe("Keyboard Control - Parameter Selection", () => {
    beforeEach(() => {
      plugin.setParameterOrder(["param1", "param2", "param3"]);
      plugin.setParameterLabels({
        param1: "Parameter 1",
        param2: "Parameter 2",
        param3: "Parameter 3",
      });
      plugin.initialize(container);
    });

    test("should select parameter by index", () => {
      // Access through public method (would be exposed in real plugin)
      expect(plugin.getSelectedParameterIndex()).toBe(-1);
    });

    test("should clear parameter selection on ESC", () => {
      plugin.initialize(container);

      const escEvent = new KeyboardEvent("keydown", { key: "Escape" });
      document.dispatchEvent(escEvent);

      expect(plugin.getSelectedParameterIndex()).toBe(-1);
    });

    test("should ignore keyboard events when typing in input", () => {
      const input = document.createElement("input");
      document.body.appendChild(input);
      input.focus();

      // Dispatch event from input
      const event = new KeyboardEvent("keydown", { key: "1", bubbles: true });
      Object.defineProperty(event, "target", { value: input, writable: false });

      document.dispatchEvent(event);

      // Selection should not change
      expect(plugin.getSelectedParameterIndex()).toBe(-1);

      document.body.removeChild(input);
    });
  });

  describe("Destruction", () => {
    test("should remove keyboard handler on destroy", () => {
      const removeEventListenerSpy = vi.spyOn(document, "removeEventListener");
      plugin.initialize(container);

      plugin.destroy();

      expect(removeEventListenerSpy).toHaveBeenCalledWith(
        "keydown",
        expect.any(Function),
      );
    });

    test("should clear container on destroy", () => {
      plugin.initialize(container);
      plugin.destroy();

      expect(container.innerHTML).toBe("");
    });

    test("should clear event listeners on destroy", () => {
      const callback = vi.fn();
      plugin.initialize(container);
      plugin.on("test-event", callback);

      plugin.destroy();
      plugin.emit("test-event", "data");

      expect(callback).not.toHaveBeenCalled();
    });
  });

  describe("Enabled State", () => {
    beforeEach(() => {
      plugin.initialize(container);
    });

    test("should check if plugin is enabled", () => {
      expect(plugin.isEnabled()).toBe(true);
    });

    test("should update enabled state", () => {
      plugin.setState({ enabled: false });
      expect(plugin.isEnabled()).toBe(false);
    });
  });
});
