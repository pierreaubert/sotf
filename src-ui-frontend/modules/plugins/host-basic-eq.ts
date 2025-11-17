// Basic EQ Host
// Preconfigured plugin host with just an EQ

import { PluginHost, type HostConfig, type HostCallbacks } from "./host";
import { EQPlugin } from "./plugin-eq";
import type { IPlugin } from "./plugin-types";

/**
 * Basic EQ Host
 * A specialized host preconfigured with just an EQ plugin
 */
export class BasicEQ {
  private host: PluginHost;
  private eqPlugin: EQPlugin;

  constructor(container: HTMLElement, callbacks: HostCallbacks = {}) {
    // Configure host with minimal UI for just EQ
    const config: HostConfig = {
      name: "Basic EQ",
      allowedPlugins: ["eq"], // Only EQ plugin
      maxPlugins: 1, // Fixed: just EQ
      showLevelMeters: true, // Show level meters
      showLUFS: true, // Show LUFS meter
      showVolumeControl: true, // Show volume control
      menubarConfig: {
        showName: true,
        showPresets: true,
        showMatrix: true, // Show matrix routing
        showMuteSolo: true, // Show mute/solo controls
      },
    };

    // Create host
    this.host = new PluginHost(container, config, callbacks);

    // Initialize EQ plugin
    this.eqPlugin = new EQPlugin();

    // Add EQ plugin to host
    this.host.addPlugin(this.eqPlugin);

    // Select EQ by default (and it's the only one)
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
   * Get all plugins (just the EQ)
   */
  getPlugins(): IPlugin[] {
    return this.host.getPlugins();
  }

  /**
   * Destroy the basic EQ
   */
  destroy(): void {
    this.host.destroy();
  }
}
