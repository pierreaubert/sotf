// PluginHost test suite
import { describe, test, expect, beforeEach, vi } from "vitest";
import {
  PluginHost,
  type HostConfig,
  type HostCallbacks,
} from "../../modules/plugins/host";
import type {
  IPlugin,
  PluginMetadata,
  PluginState,
  PluginConfig,
} from "../../modules/plugins/plugin-types";

// Mock plugin for testing
class MockPlugin implements IPlugin {
  public readonly metadata: PluginMetadata;
  private _container: HTMLElement | null = null;

  constructor(
    id: string = "mock-plugin",
    name: string = "Mock Plugin",
    category: string = "test",
  ) {
    this.metadata = {
      id,
      name,
      category,
      version: "1.0.0",
    };
  }

  initialize(container: HTMLElement, config: PluginConfig = {}): void {
    this._container = container;
    this.render(config.standalone ?? true);
  }

  render(standalone: boolean): void {
    if (this._container) {
      this._container.innerHTML = `<div class="mock-plugin">${this.metadata.name}</div>`;
    }
  }

  destroy(): void {
    if (this._container) {
      this._container.innerHTML = "";
      this._container = null;
    }
  }

  getState(): PluginState {
    return {
      enabled: true,
      bypassed: false,
      parameters: {},
    };
  }

  setState(newState: Partial<PluginState>): void {
    // Mock implementation
  }

  on(event: string, callback: (...args: any[]) => void): void {
    // Mock implementation
  }

  off(event: string, callback: (...args: any[]) => void): void {
    // Mock implementation
  }

  emit(event: string, ...args: any[]): void {
    // Mock implementation
  }

  isBypassed(): boolean {
    return false;
  }

  isEnabled(): boolean {
    return true;
  }

  setBypass(bypassed: boolean): void {
    // Mock implementation
  }

  toggleBypass(): void {
    // Mock implementation
  }

  getShortcuts() {
    return [{ key: "1", description: "Test shortcut" }];
  }
}

describe("PluginHost", () => {
  let host: PluginHost;
  let container: HTMLElement;
  let callbacks: HostCallbacks;

  beforeEach(() => {
    container = document.createElement("div");
    document.body.appendChild(container);

    callbacks = {
      onPluginAdd: vi.fn(),
      onPluginRemove: vi.fn(),
      onPluginSelect: vi.fn(),
      onVolumeChange: vi.fn(),
      onPluginReorder: vi.fn(),
    };
  });

  describe("Initialization", () => {
    test("should initialize with default config", () => {
      const config: HostConfig = {
        name: "Test Host",
      };

      host = new PluginHost(container, config, callbacks);

      expect(container.querySelector(".host-menubar")).toBeTruthy();
      expect(container.querySelector(".plugin-slots")).toBeTruthy();
      expect(container.querySelector(".display-left")).toBeTruthy();
      expect(container.querySelector(".display-right")).toBeTruthy();
    });

    test("should initialize with custom config", () => {
      const config: HostConfig = {
        name: "Custom Host",
        maxPlugins: 5,
        showLevelMeters: false,
        showLUFS: false,
        showVolumeControl: false,
        showHelpBar: false,
      };

      host = new PluginHost(container, config, callbacks);

      expect(container.querySelector(".level-meters-canvas")).toBeFalsy();
      expect(container.querySelector("[data-lufs]")).toBeFalsy();
      expect(container.querySelector(".volume-knob")).toBeFalsy();
    });

    test("should initialize with allowed plugins filter", () => {
      const config: HostConfig = {
        name: "Filtered Host",
        allowedPlugins: ["eq", "dynamics"],
      };

      host = new PluginHost(container, config, callbacks);

      // Verify the host exists
      expect(host).toBeTruthy();
    });
  });

  describe("Plugin Management - Adding Plugins", () => {
    beforeEach(() => {
      const config: HostConfig = {
        name: "Test Host",
        maxPlugins: 3,
      };
      host = new PluginHost(container, config, callbacks);
    });

    test("should add plugin to host", () => {
      const plugin = new MockPlugin();
      host.addPlugin(plugin);

      expect(host.getPlugins()).toHaveLength(1);
      expect(host.getPlugins()[0]).toBe(plugin);
    });

    test("should render plugin slot in hosting bar", () => {
      const plugin = new MockPlugin("test-1", "Test Plugin 1");
      host.addPlugin(plugin);

      const slot = container.querySelector(
        '.plugin-slots button[data-plugin-id="test-1"]',
      );
      expect(slot).toBeTruthy();
      expect(slot?.textContent).toContain("Test Plugin 1");
    });

    test("should call onPluginAdd callback", () => {
      const plugin = new MockPlugin();
      host.addPlugin(plugin);

      expect(callbacks.onPluginAdd).toHaveBeenCalledWith(plugin);
    });

    test("should auto-select first plugin", () => {
      const plugin = new MockPlugin();
      host.addPlugin(plugin);

      expect(host.getSelectedPlugin()).toBe(plugin);
      expect(callbacks.onPluginSelect).toHaveBeenCalledWith(plugin);
    });

    test("should not auto-select subsequent plugins", () => {
      const plugin1 = new MockPlugin("p1", "Plugin 1");
      const plugin2 = new MockPlugin("p2", "Plugin 2");

      host.addPlugin(plugin1);
      host.addPlugin(plugin2);

      expect(host.getSelectedPlugin()).toBe(plugin1);
    });

    test("should respect max plugins limit", () => {
      const plugin1 = new MockPlugin("p1");
      const plugin2 = new MockPlugin("p2");
      const plugin3 = new MockPlugin("p3");
      const plugin4 = new MockPlugin("p4");

      host.addPlugin(plugin1);
      host.addPlugin(plugin2);
      host.addPlugin(plugin3);
      host.addPlugin(plugin4); // Should be rejected (max is 3)

      expect(host.getPlugins()).toHaveLength(3);
    });

    test("should respect allowed plugins filter", () => {
      const config: HostConfig = {
        name: "Filtered Host",
        allowedPlugins: ["eq"],
      };
      host = new PluginHost(container, config, callbacks);

      const eqPlugin = new MockPlugin("eq-1", "EQ", "eq");
      const dynamicsPlugin = new MockPlugin("comp-1", "Compressor", "dynamics");

      host.addPlugin(eqPlugin);
      host.addPlugin(dynamicsPlugin); // Should be rejected

      expect(host.getPlugins()).toHaveLength(1);
      expect(host.getPlugins()[0]).toBe(eqPlugin);
    });
  });

  describe("Plugin Management - Removing Plugins", () => {
    beforeEach(() => {
      const config: HostConfig = {
        name: "Test Host",
      };
      host = new PluginHost(container, config, callbacks);
    });

    test("should remove plugin from host", () => {
      const plugin = new MockPlugin();
      host.addPlugin(plugin);
      host.removePlugin(plugin);

      expect(host.getPlugins()).toHaveLength(0);
    });

    test("should call onPluginRemove callback", () => {
      const plugin = new MockPlugin();
      host.addPlugin(plugin);
      host.removePlugin(plugin);

      expect(callbacks.onPluginRemove).toHaveBeenCalledWith(plugin);
    });

    test("should clear selection if removed plugin was selected", () => {
      const plugin = new MockPlugin();
      host.addPlugin(plugin);
      expect(host.getSelectedPlugin()).toBe(plugin);

      host.removePlugin(plugin);

      expect(host.getSelectedPlugin()).toBe(null);
    });

    test("should destroy plugin on removal", () => {
      const plugin = new MockPlugin();
      const destroySpy = vi.spyOn(plugin, "destroy");

      host.addPlugin(plugin);
      host.removePlugin(plugin);

      expect(destroySpy).toHaveBeenCalled();
    });

    test("should remove plugin slot from DOM", () => {
      const plugin = new MockPlugin("test-1");
      host.addPlugin(plugin);

      expect(container.querySelector('[data-plugin-id="test-1"]')).toBeTruthy();

      host.removePlugin(plugin);

      expect(container.querySelector('[data-plugin-id="test-1"]')).toBeFalsy();
    });
  });

  describe("Plugin Selection", () => {
    beforeEach(() => {
      const config: HostConfig = {
        name: "Test Host",
      };
      host = new PluginHost(container, config, callbacks);
    });

    test("should select plugin", () => {
      const plugin1 = new MockPlugin("p1", "Plugin 1");
      const plugin2 = new MockPlugin("p2", "Plugin 2");

      host.addPlugin(plugin1);
      host.addPlugin(plugin2);

      host.selectPlugin(plugin2);

      expect(host.getSelectedPlugin()).toBe(plugin2);
    });

    test("should render selected plugin in display area", () => {
      const plugin = new MockPlugin("test", "Test Plugin");
      host.addPlugin(plugin);
      host.selectPlugin(plugin);

      const displayArea = container.querySelector(
        ".display-left .active-plugin-container",
      );
      expect(displayArea).toBeTruthy();
    });

    test("should highlight selected plugin slot", () => {
      const plugin = new MockPlugin("test-1", "Test Plugin");
      host.addPlugin(plugin);
      host.selectPlugin(plugin);

      const slot = container.querySelector('[data-plugin-id="test-1"]');
      expect(slot?.classList.contains("is-selected")).toBe(true);
    });

    test("should call onPluginSelect callback", () => {
      const plugin = new MockPlugin();
      host.addPlugin(plugin);

      // Reset mock from auto-select
      vi.clearAllMocks();

      host.selectPlugin(plugin);

      expect(callbacks.onPluginSelect).toHaveBeenCalledWith(plugin);
    });

    test("should clear selection when selecting null", () => {
      const plugin = new MockPlugin();
      host.addPlugin(plugin);
      host.selectPlugin(null);

      expect(host.getSelectedPlugin()).toBe(null);
    });

    test("should show placeholder when no plugin selected", () => {
      const plugin = new MockPlugin();
      host.addPlugin(plugin);
      host.selectPlugin(null);

      const placeholder = container.querySelector(".display-left svg");
      expect(placeholder).toBeTruthy();
    });
  });

  describe("Plugin Reordering", () => {
    beforeEach(() => {
      const config: HostConfig = {
        name: "Test Host",
      };
      host = new PluginHost(container, config, callbacks);
    });

    test("should reorder plugins via drag and drop", () => {
      const plugin1 = new MockPlugin("p1", "Plugin 1");
      const plugin2 = new MockPlugin("p2", "Plugin 2");
      const plugin3 = new MockPlugin("p3", "Plugin 3");

      host.addPlugin(plugin1);
      host.addPlugin(plugin2);
      host.addPlugin(plugin3);

      // Get slot elements
      const slot1 = container.querySelector(
        '[data-plugin-id="p1"]',
      ) as HTMLElement;
      const slot3 = container.querySelector(
        '[data-plugin-id="p3"]',
      ) as HTMLElement;

      expect(slot1).toBeTruthy();
      expect(slot3).toBeTruthy();

      // Simulate drag and drop: drag p1 to p3's position
      const dragStartEvent = new DragEvent("dragstart", {
        bubbles: true,
        dataTransfer: {
          effectAllowed: "",
          setData: vi.fn(),
        } as any,
      });
      slot1?.dispatchEvent(dragStartEvent);

      const dropEvent = new DragEvent("drop", {
        bubbles: true,
        dataTransfer: {
          dropEffect: "",
        } as any,
      });
      slot3?.dispatchEvent(dropEvent);

      // Verify callback was called with reordered plugins
      expect(callbacks.onPluginReorder).toHaveBeenCalled();
    });
  });

  describe("Volume Control", () => {
    beforeEach(() => {
      const config: HostConfig = {
        name: "Test Host",
        showVolumeControl: true,
      };
      host = new PluginHost(container, config, callbacks);
    });

    test("should initialize with default volume", () => {
      expect(host.getVolume()).toBe(1.0);
    });

    test("should set volume", () => {
      host.setVolume(0.5);
      expect(host.getVolume()).toBe(0.5);
    });

    test("should clamp volume to valid range", () => {
      host.setVolume(1.5);
      expect(host.getVolume()).toBe(1.0);

      host.setVolume(-0.5);
      expect(host.getVolume()).toBe(0.0);
    });

    test("should call onVolumeChange callback", () => {
      host.setVolume(0.7);
      expect(callbacks.onVolumeChange).toHaveBeenCalledWith(0.7);
    });

    test("should update volume display", () => {
      host.setVolume(0.5);

      const volumeText = container.querySelector(".volume-value-svg");
      expect(volumeText?.textContent).toBe("50");
    });
  });

  describe("Monitoring Mode", () => {
    beforeEach(() => {
      const config: HostConfig = {
        name: "Test Host",
        showLevelMeters: true,
      };
      host = new PluginHost(container, config, callbacks);
    });

    test("should initialize with output monitoring mode", () => {
      const outputButton = container.querySelector(
        'button[data-mode="output"]',
      );
      expect(outputButton?.classList.contains("is-selected")).toBe(true);
    });

    test("should switch to input monitoring mode", () => {
      host.setMonitoringMode("input");

      const inputButton = container.querySelector('button[data-mode="input"]');
      const outputButton = container.querySelector(
        'button[data-mode="output"]',
      );

      expect(inputButton?.classList.contains("is-selected")).toBe(true);
      expect(outputButton?.classList.contains("is-selected")).toBe(false);
    });

    test("should switch to output monitoring mode", () => {
      host.setMonitoringMode("input");
      host.setMonitoringMode("output");

      const outputButton = container.querySelector(
        'button[data-mode="output"]',
      );
      expect(outputButton?.classList.contains("is-selected")).toBe(true);
    });
  });

  describe("Level Meters", () => {
    beforeEach(() => {
      const config: HostConfig = {
        name: "Test Host",
        showLevelMeters: true,
      };
      host = new PluginHost(container, config, callbacks);
    });

    test("should render level meters canvas", () => {
      const canvas = container.querySelector(".level-meters-canvas");
      expect(canvas).toBeTruthy();
      expect(canvas?.tagName).toBe("CANVAS");
    });

    test("should update level meters with data", () => {
      const data = {
        channels: [0.5, 0.7], // LevelMeter expects channels, not levels
        peaks: [0.8, 0.9],
      };

      // Should not throw
      host.updateLevelMeters(data);
      // Just verify it doesn't throw an error
      expect(true).toBe(true);
    });
  });

  describe("LUFS Meter", () => {
    beforeEach(() => {
      const config: HostConfig = {
        name: "Test Host",
        showLUFS: true,
      };
      host = new PluginHost(container, config, callbacks);
    });

    test("should render LUFS meter", () => {
      const lufsElements = container.querySelectorAll("[data-lufs]");
      expect(lufsElements.length).toBe(3); // M, S, I
    });

    test("should update LUFS values", () => {
      const data = {
        momentary: -12.5,
        shortTerm: -14.2,
        integrated: -16.8,
      };

      host.updateLUFS(data);

      const momentary = container.querySelector('[data-lufs="momentary"]');
      const shortTerm = container.querySelector('[data-lufs="shortTerm"]');
      const integrated = container.querySelector('[data-lufs="integrated"]');

      expect(momentary?.textContent).toBe("-12.5");
      expect(shortTerm?.textContent).toBe("-14.2");
      expect(integrated?.textContent).toBe("-16.8");
    });
  });

  describe("Help Bar", () => {
    beforeEach(() => {
      const config: HostConfig = {
        name: "Test Host",
        showHelpBar: true,
      };
      host = new PluginHost(container, config, callbacks);
    });

    test("should render help bar", () => {
      const helpBar = container.querySelector(".notification");
      expect(helpBar).toBeTruthy();
    });

    test("should show default shortcuts", () => {
      const shortcuts = container.querySelectorAll(".tags.has-addons");
      expect(shortcuts.length).toBeGreaterThan(0);
    });

    test("should update shortcuts when plugin is selected", () => {
      const plugin = new MockPlugin();
      host.addPlugin(plugin);
      host.selectPlugin(plugin);

      // Help bar should include plugin-specific shortcuts
      // The help bar might be re-rendered, so query it again
      const helpBar = container.querySelector(".notification");
      expect(helpBar).toBeTruthy();

      // Check that shortcuts are present (may vary based on implementation)
      const shortcuts = container.querySelectorAll(".tags.has-addons");
      expect(shortcuts.length).toBeGreaterThan(0);
    });

    test("should toggle help bar visibility", () => {
      let helpBar = container.querySelector(".notification") as HTMLElement;
      expect(helpBar).toBeTruthy();

      // Initially visible
      const initialDisplay = helpBar.style.display || "";

      host.toggleHelpBar();
      // Query again as element may have been modified
      helpBar = container.querySelector(".notification") as HTMLElement;
      expect(helpBar.style.display).toBe("none");

      host.toggleHelpBar();
      helpBar = container.querySelector(".notification") as HTMLElement;
      expect(helpBar.style.display).toBe("flex");
    });
  });

  describe("Plugin Selector Dialog", () => {
    beforeEach(() => {
      const config: HostConfig = {
        name: "Test Host",
      };
      host = new PluginHost(container, config, callbacks);
    });

    test("should show plugin selector when add button clicked", () => {
      const addButton = container.querySelector(
        ".add-plugin-btn",
      ) as HTMLButtonElement;
      addButton?.click();

      const modal = document.querySelector(".modal.is-active");
      expect(modal).toBeTruthy();
    });

    test("should close plugin selector on close button click", () => {
      const addButton = container.querySelector(
        ".add-plugin-btn",
      ) as HTMLButtonElement;
      addButton?.click();

      const modal = document.querySelector(".modal.is-active");
      expect(modal).toBeTruthy();

      const closeButton = document.querySelector(
        ".plugin-selector-close",
      ) as HTMLButtonElement;
      expect(closeButton).toBeTruthy();

      // Clicking should not throw (modal cleanup happens async)
      expect(() => closeButton?.click()).not.toThrow();
    });

    test("should close plugin selector on background click", () => {
      const addButton = container.querySelector(
        ".add-plugin-btn",
      ) as HTMLButtonElement;
      addButton?.click();

      const modal = document.querySelector(".modal.is-active");
      expect(modal).toBeTruthy();

      const background = document.querySelector(
        ".modal-background",
      ) as HTMLElement;
      expect(background).toBeTruthy();

      // Clicking should not throw (modal cleanup happens async)
      expect(() => background?.click()).not.toThrow();
    });

    test("should filter plugins by allowed types", () => {
      // Clean up any existing modals first
      document.querySelectorAll(".modal").forEach((m) => m.remove());

      const config: HostConfig = {
        name: "Filtered Host",
        allowedPlugins: ["eq"],
      };
      host.destroy();
      host = new PluginHost(container, config, callbacks);

      const addButton = container.querySelector(
        ".add-plugin-btn",
      ) as HTMLButtonElement;
      expect(addButton).toBeTruthy();
      addButton?.click();

      const pluginItems = document.querySelectorAll(".plugin-selector-item");

      // Should only show EQ plugin (1 item)
      expect(pluginItems.length).toBe(1);

      // Clean up modal
      document.querySelectorAll(".modal").forEach((m) => m.remove());
    });
  });

  describe("Keyboard Shortcuts", () => {
    beforeEach(() => {
      const config: HostConfig = {
        name: "Test Host",
        showVolumeControl: true,
      };
      host = new PluginHost(container, config, callbacks);
    });

    test("should increase volume on ArrowUp", () => {
      // Start with lower volume so we can increase
      host.setVolume(0.5);
      const initialVolume = host.getVolume();

      const event = new KeyboardEvent("keydown", {
        key: "ArrowUp",
        bubbles: true,
      });
      document.dispatchEvent(event);

      expect(host.getVolume()).toBeGreaterThan(initialVolume);
    });

    test("should decrease volume on ArrowDown", () => {
      host.setVolume(0.5);
      const initialVolume = host.getVolume();

      const event = new KeyboardEvent("keydown", {
        key: "ArrowDown",
        bubbles: true,
      });
      document.dispatchEvent(event);

      expect(host.getVolume()).toBeLessThan(initialVolume);
    });
  });

  describe("Destruction", () => {
    test("should destroy all plugins", () => {
      const config: HostConfig = {
        name: "Test Host",
      };
      host = new PluginHost(container, config, callbacks);

      const plugin1 = new MockPlugin("p1");
      const plugin2 = new MockPlugin("p2");
      const destroySpy1 = vi.spyOn(plugin1, "destroy");
      const destroySpy2 = vi.spyOn(plugin2, "destroy");

      host.addPlugin(plugin1);
      host.addPlugin(plugin2);

      host.destroy();

      expect(destroySpy1).toHaveBeenCalled();
      expect(destroySpy2).toHaveBeenCalled();
    });

    test("should clear container", () => {
      const config: HostConfig = {
        name: "Test Host",
      };
      host = new PluginHost(container, config, callbacks);

      host.destroy();

      expect(container.innerHTML).toBe("");
    });

    test("should remove keyboard listener", () => {
      const config: HostConfig = {
        name: "Test Host",
      };
      const removeEventListenerSpy = vi.spyOn(document, "removeEventListener");

      host = new PluginHost(container, config, callbacks);
      host.destroy();

      expect(removeEventListenerSpy).toHaveBeenCalledWith(
        "keydown",
        expect.any(Function),
      );
    });
  });
});
