// Global type declarations

// Extend Window interface for Tauri internals
declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

// Export empty object to make this a module
export {};
