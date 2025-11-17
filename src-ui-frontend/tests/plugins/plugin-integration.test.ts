// Plugin System Integration Tests
// Tests the complete workflow of plugin management, selection, and interaction
import { describe, test, expect, beforeEach, vi } from "vitest";
import { PluginHost, type HostConfig } from "../../modules/plugins/host";
import { EQPlugin } from "../../modules/plugins/plugin-eq";
import { CompressorPlugin } from "../../modules/plugins/plugin-compressor";
import { LimiterPlugin } from "../../modules/plugins/plugin-limiter";
import { SpectrumPlugin } from "../../modules/plugins/plugin-spectrum";
import { UpmixerPlugin } from "../../modules/plugins/plugin-upmixer";

describe("Plugin System Integration", () => {
  let container: HTMLElement;

  beforeEach(() => {
    container = document.createElement("div");
    document.body.appendChild(container);
  });

  describe("Complete Plugin Lifecycle", () => {
    test("should add, select, configure, and remove EQ plugin", () => {
      const config: HostConfig = {
        name: "Integration Test Host",
      };
      const host = new PluginHost(container, config);

      // Add EQ plugin
      const eqPlugin = new EQPlugin();
      host.addPlugin(eqPlugin);

      // Verify it's added and selected
      expect(host.getPlugins()).toHaveLength(1);
      expect(host.getSelectedPlugin()).toBe(eqPlugin);

      // Verify it's rendered in display area
      const displayArea = container.querySelector(
        ".display-left .active-plugin-container",
      );
      expect(displayArea).toBeTruthy();

      // Verify slot is rendered
      const slot = container.querySelector('[data-plugin-id="eq-plugin"]');
      expect(slot).toBeTruthy();

      // Remove plugin
      host.removePlugin(eqPlugin);

      // Verify it's removed
      expect(host.getPlugins()).toHaveLength(0);
      expect(host.getSelectedPlugin()).toBe(null);
      expect(
        container.querySelector('[data-plugin-id="eq-plugin"]'),
      ).toBeFalsy();
    });

    test("should handle multiple plugins in sequence", () => {
      const host = new PluginHost(container, { name: "Multi Plugin Host" });

      const eq = new EQPlugin();
      const comp = new CompressorPlugin();
      const limiter = new LimiterPlugin();

      // Add plugins
      host.addPlugin(eq);
      host.addPlugin(comp);
      host.addPlugin(limiter);

      // Verify all are added
      expect(host.getPlugins()).toHaveLength(3);

      // Verify slots are rendered in order
      const slots = container.querySelectorAll(".plugin-slots button");
      expect(slots).toHaveLength(3);

      // Select second plugin
      host.selectPlugin(comp);
      expect(host.getSelectedPlugin()).toBe(comp);

      // Remove middle plugin
      host.removePlugin(comp);
      expect(host.getPlugins()).toHaveLength(2);
      expect(host.getPlugins()).toEqual([eq, limiter]);
    });
  });

  describe("Plugin Type-Specific Workflows", () => {
    test("should handle EQ plugin configuration", () => {
      const host = new PluginHost(container, { name: "EQ Test Host" });
      const eq = new EQPlugin();

      host.addPlugin(eq);
      host.selectPlugin(eq);

      // Verify EQ table is rendered
      const eqTable = container.querySelector(".eq-table");
      expect(eqTable).toBeTruthy();

      // Verify filter controls are present
      const filterInputs = container.querySelectorAll(".filter-frequency");
      expect(filterInputs.length).toBeGreaterThan(0);
    });

    test("should handle Compressor plugin with sliders", () => {
      const host = new PluginHost(container, { name: "Compressor Test Host" });
      const comp = new CompressorPlugin();

      host.addPlugin(comp);
      host.selectPlugin(comp);

      // Verify parameter sliders are rendered
      const sliders = container.querySelectorAll(".param-slider");
      expect(sliders.length).toBeGreaterThan(0);

      // Verify parameter fields
      const paramFields = container.querySelectorAll(".parameter-field");
      expect(paramFields.length).toBe(6); // threshold, ratio, attack, release, knee, makeupGain
    });

    test("should handle Limiter plugin with reduced height sliders", () => {
      const host = new PluginHost(container, { name: "Limiter Test Host" });
      const limiter = new LimiterPlugin();

      host.addPlugin(limiter);
      host.selectPlugin(limiter);

      // Verify sliders are present
      const sliders = container.querySelectorAll(".param-slider");
      expect(sliders.length).toBe(3); // ceiling, release, lookahead

      // Verify sliders have correct height (250px)
      const slider = sliders[0] as HTMLElement;
      expect(slider.style.height).toBe("250px");
    });

    test("should handle Spectrum plugin with canvas", () => {
      const host = new PluginHost(container, { name: "Spectrum Test Host" });
      const spectrum = new SpectrumPlugin();

      host.addPlugin(spectrum);
      host.selectPlugin(spectrum);

      // Verify canvas is rendered
      const canvas = container.querySelector(".spectrum-canvas");
      expect(canvas).toBeTruthy();
      expect(canvas?.tagName).toBe("CANVAS");

      // Verify control buttons
      const startBtn = container.querySelector(".start-btn");
      const stopBtn = container.querySelector(".stop-btn");
      expect(startBtn).toBeTruthy();
      expect(stopBtn).toBeTruthy();
    });

    test("should handle Upmixer plugin configuration", () => {
      const host = new PluginHost(container, { name: "Upmixer Test Host" });
      const upmixer = new UpmixerPlugin();

      host.addPlugin(upmixer);
      host.selectPlugin(upmixer);

      // Verify parameter fields
      const paramFields = container.querySelectorAll(".parameter-field");
      expect(paramFields.length).toBe(4); // centerLevel, surroundLevel, lfeLevel, crossfeedAmount
    });
  });

  describe("Plugin Keyboard Controls Integration", () => {
    test("should handle keyboard selection across plugins", () => {
      const host = new PluginHost(container, { name: "Keyboard Test Host" });
      const comp = new CompressorPlugin();

      host.addPlugin(comp);
      host.selectPlugin(comp);

      // Simulate pressing '1' to select first parameter
      const event = new KeyboardEvent("keydown", { key: "1", bubbles: true });
      document.dispatchEvent(event);

      // Plugin should handle this internally
      // We can verify the parameter field gets highlighted
      // Note: This requires the plugin to be fully rendered
    });

    test("should handle Tab navigation between parameters", () => {
      const host = new PluginHost(container, { name: "Tab Test Host" });
      const limiter = new LimiterPlugin();

      host.addPlugin(limiter);
      host.selectPlugin(limiter);

      // Simulate Tab key
      const tabEvent = new KeyboardEvent("keydown", {
        key: "Tab",
        bubbles: true,
      });
      document.dispatchEvent(tabEvent);

      // Parameters should cycle
    });

    test("should handle Shift+Up/Down parameter adjustment", () => {
      const host = new PluginHost(container, { name: "Adjust Test Host" });
      const comp = new CompressorPlugin();

      host.addPlugin(comp);
      host.selectPlugin(comp);

      // Select parameter first
      const selectEvent = new KeyboardEvent("keydown", {
        key: "1",
        bubbles: true,
      });
      document.dispatchEvent(selectEvent);

      // Adjust with Shift+Up
      const adjustEvent = new KeyboardEvent("keydown", {
        key: "ArrowUp",
        shiftKey: true,
        bubbles: true,
      });
      document.dispatchEvent(adjustEvent);

      // Parameter value should change
    });
  });

  describe("Plugin State Persistence", () => {
    test("should maintain plugin state after selection changes", () => {
      const host = new PluginHost(container, { name: "State Test Host" });
      const eq = new EQPlugin();
      const comp = new CompressorPlugin();

      host.addPlugin(eq);
      host.addPlugin(comp);

      // Select EQ, make changes
      host.selectPlugin(eq);
      const eqState = eq.getState();

      // Switch to compressor
      host.selectPlugin(comp);

      // Switch back to EQ
      host.selectPlugin(eq);

      // State should be preserved
      expect(eq.getState()).toEqual(eqState);
    });

    test("should handle bypass state across plugins", () => {
      const host = new PluginHost(container, { name: "Bypass Test Host" });
      const eq = new EQPlugin();
      const comp = new CompressorPlugin();

      host.addPlugin(eq);
      host.addPlugin(comp);

      // Bypass EQ
      eq.setBypass(true);
      expect(eq.isBypassed()).toBe(true);

      // Compressor should not be bypassed
      expect(comp.isBypassed()).toBe(false);

      // Switch plugins
      host.selectPlugin(comp);
      host.selectPlugin(eq);

      // EQ should still be bypassed
      expect(eq.isBypassed()).toBe(true);
    });
  });

  describe("Plugin Chain Workflow", () => {
    test("should create typical mastering chain", () => {
      const host = new PluginHost(container, { name: "Mastering Chain" });

      // Typical mastering chain: EQ → Compressor → Limiter
      const eq = new EQPlugin();
      const comp = new CompressorPlugin();
      const limiter = new LimiterPlugin();

      host.addPlugin(eq);
      host.addPlugin(comp);
      host.addPlugin(limiter);

      expect(host.getPlugins()).toEqual([eq, comp, limiter]);

      // Verify all slots are visible
      const slots = container.querySelectorAll(".plugin-slots button");
      expect(slots).toHaveLength(3);
    });

    test("should create spatial audio chain", () => {
      const host = new PluginHost(container, { name: "Spatial Chain" });

      // Spatial chain: EQ → Upmixer → EQ (for center channel)
      const eqStereo = new EQPlugin();
      const upmixer = new UpmixerPlugin();
      const eqSurround = new EQPlugin();

      host.addPlugin(eqStereo);
      host.addPlugin(upmixer);
      host.addPlugin(eqSurround);

      expect(host.getPlugins()).toHaveLength(3);
    });

    test("should create analysis chain", () => {
      const host = new PluginHost(container, { name: "Analysis Chain" });

      // Analysis chain: Spectrum analyzer
      const spectrum = new SpectrumPlugin();

      host.addPlugin(spectrum);
      host.selectPlugin(spectrum);

      expect(host.getPlugins()).toHaveLength(1);
    });
  });

  describe("Plugin Removal Edge Cases", () => {
    test("should handle removing first plugin in chain", () => {
      const host = new PluginHost(container, { name: "Remove First Test" });

      const p1 = new EQPlugin();
      const p2 = new CompressorPlugin();
      const p3 = new LimiterPlugin();

      host.addPlugin(p1);
      host.addPlugin(p2);
      host.addPlugin(p3);

      host.removePlugin(p1);

      expect(host.getPlugins()).toEqual([p2, p3]);
    });

    test("should handle removing last plugin in chain", () => {
      const host = new PluginHost(container, { name: "Remove Last Test" });

      const p1 = new EQPlugin();
      const p2 = new CompressorPlugin();
      const p3 = new LimiterPlugin();

      host.addPlugin(p1);
      host.addPlugin(p2);
      host.addPlugin(p3);

      host.removePlugin(p3);

      expect(host.getPlugins()).toEqual([p1, p2]);
    });

    test("should handle removing middle plugin in chain", () => {
      const host = new PluginHost(container, { name: "Remove Middle Test" });

      const p1 = new EQPlugin();
      const p2 = new CompressorPlugin();
      const p3 = new LimiterPlugin();

      host.addPlugin(p1);
      host.addPlugin(p2);
      host.addPlugin(p3);

      host.removePlugin(p2);

      expect(host.getPlugins()).toEqual([p1, p3]);
    });

    test("should handle removing all plugins", () => {
      const host = new PluginHost(container, { name: "Remove All Test" });

      const p1 = new EQPlugin();
      const p2 = new CompressorPlugin();

      host.addPlugin(p1);
      host.addPlugin(p2);

      host.removePlugin(p1);
      host.removePlugin(p2);

      expect(host.getPlugins()).toHaveLength(0);
      expect(host.getSelectedPlugin()).toBe(null);
    });
  });

  describe("Plugin Limits and Restrictions", () => {
    test("should enforce max plugins limit", () => {
      const host = new PluginHost(container, {
        name: "Limited Host",
        maxPlugins: 2,
      });

      const p1 = new EQPlugin();
      const p2 = new CompressorPlugin();
      const p3 = new LimiterPlugin();

      host.addPlugin(p1);
      host.addPlugin(p2);
      host.addPlugin(p3); // Should be rejected

      expect(host.getPlugins()).toHaveLength(2);
    });

    test("should enforce allowed plugin types", () => {
      const host = new PluginHost(container, {
        name: "Filtered Host",
        allowedPlugins: ["eq", "dynamics"],
      });

      const eq = new EQPlugin(); // category: 'eq'
      const comp = new CompressorPlugin(); // category: 'dynamics'
      const spectrum = new SpectrumPlugin(); // category: 'analyzer'

      host.addPlugin(eq);
      host.addPlugin(comp);
      host.addPlugin(spectrum); // Should be rejected

      expect(host.getPlugins()).toHaveLength(2);
      expect(host.getPlugins()).toContain(eq);
      expect(host.getPlugins()).toContain(comp);
      expect(host.getPlugins()).not.toContain(spectrum);
    });
  });

  describe("Plugin Event Handling", () => {
    test("should handle plugin state change events", () => {
      const host = new PluginHost(container, { name: "Event Test Host" });
      const eq = new EQPlugin();

      const stateChangeCallback = vi.fn();
      eq.on("stateChanged", stateChangeCallback);

      host.addPlugin(eq);

      eq.setState({ bypassed: true });

      expect(stateChangeCallback).toHaveBeenCalled();
    });

    test("should handle plugin parameter change events", () => {
      const host = new PluginHost(container, { name: "Param Event Host" });
      const comp = new CompressorPlugin();

      const paramChangeCallback = vi.fn();
      comp.on("parameterChanged", paramChangeCallback);

      host.addPlugin(comp);

      // Trigger parameter change through state
      comp.setState({
        parameters: {
          threshold: -20,
        },
      });

      expect(paramChangeCallback).toHaveBeenCalled();
    });
  });

  describe("Regression Tests", () => {
    test("should prevent regression: plugins appear in bar after adding", () => {
      const host = new PluginHost(container, { name: "Regression Test 1" });
      const eq = new EQPlugin();

      host.addPlugin(eq);

      // Verify slot exists in DOM
      const slot = container.querySelector(
        '.plugin-slots button[data-plugin-id="eq-plugin"]',
      );
      expect(slot).toBeTruthy();
      expect(slot?.textContent).toContain("EQ");
    });

    test("should prevent regression: drag and drop works for reordering", () => {
      const host = new PluginHost(container, { name: "Regression Test 2" });

      const p1 = new EQPlugin();
      const p2 = new CompressorPlugin();

      host.addPlugin(p1);
      host.addPlugin(p2);

      // Get slots
      const slots = container.querySelectorAll(".plugin-slots button");
      expect(slots).toHaveLength(2);

      // Verify draggable attribute
      expect((slots[0] as HTMLElement).draggable).toBe(true);
      expect((slots[1] as HTMLElement).draggable).toBe(true);
    });

    test("should prevent regression: keyboard controls work across all plugins", () => {
      const host = new PluginHost(container, { name: "Regression Test 3" });

      const eq = new EQPlugin();
      const comp = new CompressorPlugin();
      const limiter = new LimiterPlugin();
      const upmixer = new UpmixerPlugin();
      const spectrum = new SpectrumPlugin();

      // All plugins should initialize without error
      expect(() => {
        host.addPlugin(eq);
        host.addPlugin(comp);
        host.addPlugin(limiter);
        host.addPlugin(upmixer);
        host.addPlugin(spectrum);
      }).not.toThrow();

      // All should respond to keyboard events
      [eq, comp, limiter, upmixer, spectrum].forEach((plugin) => {
        expect(plugin.getShortcuts).toBeDefined();
        expect(plugin.getShortcuts().length).toBeGreaterThan(0);
      });
    });

    test("should prevent regression: modal appears above plugins", () => {
      const host = new PluginHost(container, { name: "Regression Test 4" });

      const addButton = container.querySelector(
        ".add-plugin-btn",
      ) as HTMLButtonElement;
      addButton?.click();

      const modal = document.querySelector(".modal.is-active") as HTMLElement;
      expect(modal).toBeTruthy();

      // Verify z-index is set
      expect(modal.style.zIndex).toBe("9999");
    });

    test("should prevent regression: EQ table has dark theme", () => {
      const host = new PluginHost(container, { name: "Regression Test 5" });
      const eq = new EQPlugin();

      host.addPlugin(eq);
      host.selectPlugin(eq);

      // Check table exists
      const table = container.querySelector(".eq-table");
      expect(table).toBeTruthy();

      // Check table header background
      const thead = container.querySelector(".eq-table thead");
      expect(thead?.classList.contains("has-background-grey-dark")).toBe(true);

      // Check that table rows have dark backgrounds
      const rows = container.querySelectorAll(".eq-table tbody tr");
      if (rows.length > 0) {
        // At least one row should have a dark background class
        const hasDarkTheme = Array.from(rows).some(
          (row) =>
            row.classList.contains("has-background-black-ter") ||
            row.classList.contains("has-background-black-bis"),
        );
        expect(hasDarkTheme).toBe(true);
      }
    });
  });
});
