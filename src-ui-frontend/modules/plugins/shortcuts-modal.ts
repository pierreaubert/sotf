// Reusable Shortcuts Modal Component
// Displays keyboard shortcuts for plugins and host

export interface ShortcutItem {
  key: string;
  description: string;
}

export interface ShortcutsModalConfig {
  hostShortcuts?: ShortcutItem[];
  pluginShortcuts?: ShortcutItem[];
  pluginName?: string;
}

/**
 * ShortcutsModal
 * Reusable modal for displaying keyboard shortcuts
 */
export class ShortcutsModal {
  private modalElement: HTMLElement | null = null;
  private config: ShortcutsModalConfig;
  private onCloseCallback?: () => void;

  constructor(config: ShortcutsModalConfig = {}) {
    this.config = config;
  }

  /**
   * Create and return the modal HTML element
   */
  createModal(): HTMLElement {
    const modal = document.createElement("div");
    modal.className = "modal shortcuts-modal";
    modal.innerHTML = this.generateModalHTML();
    this.modalElement = modal;
    this.attachEventListeners();
    return modal;
  }

  /**
   * Generate modal HTML using Bulma tags
   */
  private generateModalHTML(): string {
    const pluginName = this.config.pluginName || "Plugin";
    const hasPluginShortcuts =
      this.config.pluginShortcuts && this.config.pluginShortcuts.length > 0;
    const hasHostShortcuts =
      this.config.hostShortcuts && this.config.hostShortcuts.length > 0;

    return `
      <div class="modal-background"></div>
      <div class="modal-card">
        <header class="modal-card-head">
          <p class="modal-card-title">Keyboard Shortcuts</p>
          <button class="delete shortcuts-modal-close" aria-label="close"></button>
        </header>
        <section class="modal-card-body">
          <div class="content">
            ${
              hasPluginShortcuts
                ? `
              <h5 class="title is-5">${pluginName} Shortcuts</h5>
              <div class="field is-grouped is-grouped-multiline mb-5">
                ${this.config
                  .pluginShortcuts!.map(
                    (shortcut) => `
                  <div class="control">
                    <div class="tags has-addons">
                      <span class="tag is-dark">${this.escapeHtml(shortcut.key)}</span>
                      <span class="tag is-light">${this.escapeHtml(shortcut.description)}</span>
                    </div>
                  </div>
                `,
                  )
                  .join("")}
              </div>
            `
                : ""
            }

            ${
              hasHostShortcuts
                ? `
              <h5 class="title is-5">Host Shortcuts</h5>
              <div class="field is-grouped is-grouped-multiline">
                ${this.config
                  .hostShortcuts!.map(
                    (shortcut) => `
                  <div class="control">
                    <div class="tags has-addons">
                      <span class="tag is-dark">${this.escapeHtml(shortcut.key)}</span>
                      <span class="tag is-light">${this.escapeHtml(shortcut.description)}</span>
                    </div>
                  </div>
                `,
                  )
                  .join("")}
              </div>
            `
                : ""
            }
          </div>
        </section>
        <footer class="modal-card-foot">
          <p class="has-text-grey-light is-size-7">Press <code>ESC</code> or <code>Enter</code> to close</p>
        </footer>
      </div>
    `;
  }

  /**
   * Attach event listeners to modal
   */
  private attachEventListeners(): void {
    if (!this.modalElement) return;

    // Close button
    const closeBtn = this.modalElement.querySelector(".shortcuts-modal-close");
    if (closeBtn) {
      closeBtn.addEventListener("click", () => this.hide());
    }

    // Background click
    const background = this.modalElement.querySelector(".modal-background");
    if (background) {
      background.addEventListener("click", () => this.hide());
    }

    // Keyboard handlers
    this.handleKeydown = this.handleKeydown.bind(this);
    document.addEventListener("keydown", this.handleKeydown);
  }

  /**
   * Handle keyboard events
   */
  private handleKeydown(e: KeyboardEvent): void {
    if (!this.modalElement?.classList.contains("is-active")) return;

    if (e.key === "Escape" || e.key === "Enter") {
      e.preventDefault();
      this.hide();
    }
  }

  /**
   * Show the modal
   */
  show(): void {
    if (this.modalElement) {
      this.modalElement.classList.add("is-active");
    }
  }

  /**
   * Hide the modal
   */
  hide(): void {
    if (this.modalElement) {
      this.modalElement.classList.remove("is-active");
      if (this.onCloseCallback) {
        this.onCloseCallback();
      }
    }
  }

  /**
   * Check if modal is currently shown
   */
  isVisible(): boolean {
    return this.modalElement?.classList.contains("is-active") ?? false;
  }

  /**
   * Update modal configuration and regenerate
   */
  updateConfig(config: ShortcutsModalConfig): void {
    this.config = { ...this.config, ...config };
    if (this.modalElement) {
      const wasVisible = this.isVisible();
      this.modalElement.innerHTML = this.generateModalHTML();
      this.attachEventListeners();
      if (wasVisible) {
        this.show();
      }
    }
  }

  /**
   * Set callback for when modal is closed
   */
  onClose(callback: () => void): void {
    this.onCloseCallback = callback;
  }

  /**
   * Destroy the modal and clean up
   */
  destroy(): void {
    document.removeEventListener("keydown", this.handleKeydown);
    if (this.modalElement) {
      this.modalElement.remove();
      this.modalElement = null;
    }
  }

  /**
   * Escape HTML to prevent XSS
   */
  private escapeHtml(text: string): string {
    const div = document.createElement("div");
    div.textContent = text;
    return div.innerHTML;
  }
}
