// Plugin System Exports
// Central export file for all plugin-related modules

// Types and interfaces
export * from "./plugin-types";

// Base classes
export { BasePlugin } from "./plugin-base";

// Components
export { LevelMeter, type LevelMeterConfig } from "./level-meter";
export { PluginMenubar, type PluginMenubarCallbacks } from "./plugin-menubar";

// Host
export { PluginHost, type HostConfig, type HostCallbacks } from "./host";
export { ChannelStrip } from "./host-channel-strip";
export { BasicEQ } from "./host-basic-eq";
export { HostAll } from "./host-all";

// Plugins
export { EQPlugin, type FilterParam } from "./plugin-eq";
export { UpmixerPlugin } from "./plugin-upmixer";
export { CompressorPlugin } from "./plugin-compressor";
export { LimiterPlugin } from "./plugin-limiter";
export { SpectrumPlugin } from "./plugin-spectrum";
