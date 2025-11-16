// Channel Strip
// Preconfigured plugin host with: EQ → Compressor → Limiter

import { PluginHost, type HostConfig, type HostCallbacks } from "./host";
import { EQPlugin } from "./plugin-eq";
import { CompressorPlugin } from "./plugin-compressor";
import { LimiterPlugin } from "./plugin-limiter";
import type { IPlugin } from "./plugin-types";

/**
 * Channel Strip
 * A specialized host preconfigured with a standard audio processing chain:
 * EQ → Compressor → Limiter
 */
export class ChannelStrip {
  private host: PluginHost;
  private eqPlugin: EQPlugin;
  private compressorPlugin: CompressorPlugin;
  private limiterPlugin: LimiterPlugin;

  constructor(container: HTMLElement, callbacks: HostCallbacks = {}) {
    // Configure host with fixed plugin chain
    const config: HostConfig = {
      name: "Channel Strip",
      allowedPlugins: ["eq", "dynamics"], // Only EQ and dynamics plugins
      maxPlugins: 3, // Fixed: EQ, Compressor, Limiter
      showLevelMeters: true,
      showLUFS: true,
      showVolumeControl: true,
      menubarConfig: {
        showName: true,
        showPresets: true,
        showMatrix: true,
        showMuteSolo: true,
      },
    };

    // Create host
    this.host = new PluginHost(container, config, callbacks);

    // Initialize plugins
    this.eqPlugin = new EQPlugin();
    this.compressorPlugin = new CompressorPlugin();
    this.limiterPlugin = new LimiterPlugin();

    // Add plugins to host in order: EQ → Compressor → Limiter
    this.host.addPlugin(this.eqPlugin);
    this.host.addPlugin(this.compressorPlugin);
    this.host.addPlugin(this.limiterPlugin);

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
   * Get the Compressor plugin
   */
  getCompressor(): CompressorPlugin {
    return this.compressorPlugin;
  }

  /**
   * Get the Limiter plugin
   */
  getLimiter(): LimiterPlugin {
    return this.limiterPlugin;
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
   * Destroy the channel strip
   */
  destroy(): void {
    this.host.destroy();
  }
}
