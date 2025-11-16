// All Plugins Host
// Preconfigured plugin host with EQ by default and all plugins accepted

import { PluginHost, type HostConfig, type HostCallbacks } from "./host";
import { EQPlugin } from "./plugin-eq";
import type { IPlugin } from "./plugin-types";

/**
 * All Plugins Host
 * A specialized host preconfigured with an EQ plugin by default,
 * but accepts all plugin types for maximum flexibility
 */
export class HostAll {
  private host: PluginHost;
  private eqPlugin: EQPlugin;

  constructor(container: HTMLElement, callbacks: HostCallbacks = {}) {
    // Configure host with all features and all plugins allowed
    const config: HostConfig = {
      name: "Audio Processor",
      allowedPlugins: [], // Empty = all plugins allowed
      maxPlugins: Infinity, // No limit on number of plugins
      showLevelMeters: true, // Show level meters
      showLUFS: true, // Show LUFS meter
      showVolumeControl: true, // Show volume control
      showHelpBar: true, // Show help bar with shortcuts
      menubarConfig: {
        showName: true,
        showPresets: true,
        showMatrix: true, // Show matrix routing
        showMuteSolo: true, // Show mute/solo controls
      },
    };

    // Create host
    this.host = new PluginHost(container, config, callbacks);

    // Initialize EQ plugin as default
    this.eqPlugin = new EQPlugin();

    // Add EQ plugin to host
    this.host.addPlugin(this.eqPlugin);

    // Select EQ by default
    this.host.selectPlugin(this.eqPlugin);
  }

  /**
   * Get the EQ plugin
   */
  getEQ(): EQPlugin {
    return this.eqPlugin;
  }

  /**
   * Get the host
   */
  getHost(): PluginHost {
    return this.host;
  }

  /**
   * Get all plugins
   */
  getPlugins(): IPlugin[] {
    return this.host.getPlugins();
  }

  /**
   * Add a plugin to the host
   */
  addPlugin(plugin: IPlugin): void {
    this.host.addPlugin(plugin);
  }

  /**
   * Remove a plugin from the host
   */
  removePlugin(plugin: IPlugin): void {
    this.host.removePlugin(plugin);
  }

  /**
   * Select a plugin
   */
  selectPlugin(plugin: IPlugin | null): void {
    this.host.selectPlugin(plugin);
  }

  /**
   * Get selected plugin
   */
  getSelectedPlugin(): IPlugin | null {
    return this.host.getSelectedPlugin();
  }

  /**
   * Destroy the host
   */
  destroy(): void {
    this.host.destroy();
  }
}
