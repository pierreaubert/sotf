// Saving Step Component
// Export EQ settings and measurement data

export type ExportFormat =
  | "apo"
  | "aupreset"
  | "rme-channel"
  | "rme-room"
  | "csv";

export interface SavingStepConfig {
  onStartNew?: () => void;
}

export class SavingStep {
  private container: HTMLElement;
  private config: SavingStepConfig;
  private exportedFormats: Set<ExportFormat> = new Set();

  constructor(container: HTMLElement, config: SavingStepConfig = {}) {
    this.container = container;
    this.config = config;
    this.render();
    this.attachEventListeners();
  }

  /**
   * Render the saving step UI
   */
  private render(): void {
    this.container.classList.add("saving-step");
    this.container.innerHTML = this.generateHTML();
  }

  /**
   * Generate HTML for the saving step
   */
  private generateHTML(): string {
    return `
      <div class="step-content-wrapper">
        <div class="step-header-section">
          <h2 class="step-title">Save & Export</h2>
          <p class="step-description">
            Export your optimized EQ settings to use with your favorite audio software or hardware.
          </p>
        </div>

        <div class="saving-content">
          <!-- Success Banner -->
          <div class="success-banner">
            <div class="success-icon">✓</div>
            <div class="success-text">
              <h3>Optimization Complete!</h3>
              <p>Your EQ has been successfully optimized and tested. Choose your export format below.</p>
            </div>
          </div>

          <!-- Results Summary -->
          <div class="results-summary-card">
            <h3>Optimization Summary</h3>
            <div class="summary-stats">
              <div class="stat-item">
                <label>Score Before</label>
                <span class="stat-value">3.45</span>
              </div>
              <div class="stat-item">
                <label>Score After</label>
                <span class="stat-value highlight">7.82</span>
              </div>
              <div class="stat-item">
                <label>Improvement</label>
                <span class="stat-value success">+4.37 (+126%)</span>
              </div>
              <div class="stat-item">
                <label>Filters Used</label>
                <span class="stat-value">8</span>
              </div>
            </div>
          </div>

          <!-- Export Options -->
          <div class="export-section">
            <h3>Export Formats</h3>
            <p class="export-description">
              Choose the format that matches your audio setup. You can export to multiple formats.
            </p>

            <div class="export-grid">
              <!-- APO Format -->
              <div class="export-card" data-format="apo">
                <div class="export-card-header">
                  <div class="export-icon">🎚️</div>
                  <h4>Equalizer APO</h4>
                </div>
                <p class="export-card-description">
                  For Equalizer APO on Windows. Includes all parametric EQ filters.
                </p>
                <div class="export-card-footer">
                  <button class="export-btn" data-format="apo">
                    Download APO
                  </button>
                  <span class="export-status" style="display: none">✓ Exported</span>
                </div>
              </div>

              <!-- AUPreset Format -->
              <div class="export-card" data-format="aupreset">
                <div class="export-card-header">
                  <div class="export-icon">🍎</div>
                  <h4>macOS AUPreset</h4>
                </div>
                <p class="export-card-description">
                  For macOS Audio Units. Compatible with AUNBandEQ and similar plugins.
                </p>
                <div class="export-card-footer">
                  <button class="export-btn" data-format="aupreset">
                    Download AUPreset
                  </button>
                  <span class="export-status" style="display: none">✓ Exported</span>
                </div>
              </div>

              <!-- RME Channel -->
              <div class="export-card" data-format="rme-channel">
                <div class="export-card-header">
                  <div class="export-icon">🎛️</div>
                  <h4>RME Channel EQ</h4>
                </div>
                <p class="export-card-description">
                  For RME audio interfaces (channel-based EQ). TotalMix FX compatible.
                </p>
                <div class="export-card-footer">
                  <button class="export-btn" data-format="rme-channel">
                    Download RME Channel
                  </button>
                  <span class="export-status" style="display: none">✓ Exported</span>
                </div>
              </div>

              <!-- RME Room -->
              <div class="export-card" data-format="rme-room">
                <div class="export-card-header">
                  <div class="export-icon">🏠</div>
                  <h4>RME Room EQ</h4>
                </div>
                <p class="export-card-description">
                  For RME audio interfaces (room correction). Advanced TotalMix FX feature.
                </p>
                <div class="export-card-footer">
                  <button class="export-btn" data-format="rme-room">
                    Download RME Room
                  </button>
                  <span class="export-status" style="display: none">✓ Exported</span>
                </div>
              </div>

              <!-- CSV Data -->
              <div class="export-card" data-format="csv">
                <div class="export-card-header">
                  <div class="export-icon">📊</div>
                  <h4>CSV Data</h4>
                </div>
                <p class="export-card-description">
                  Raw measurement and EQ data in CSV format. For analysis or custom processing.
                </p>
                <div class="export-card-footer">
                  <button class="export-btn" data-format="csv">
                    Download CSV
                  </button>
                  <span class="export-status" style="display: none">✓ Exported</span>
                </div>
              </div>

              <!-- Download All -->
              <div class="export-card highlight">
                <div class="export-card-header">
                  <div class="export-icon">📦</div>
                  <h4>All Formats</h4>
                </div>
                <p class="export-card-description">
                  Download all export formats in a single ZIP file for convenience.
                </p>
                <div class="export-card-footer">
                  <button class="export-btn primary" id="download_all_btn">
                    Download All
                  </button>
                </div>
              </div>
            </div>
          </div>

          <!-- Additional Options -->
          <div class="additional-options">
            <h3>Additional Actions</h3>
            <div class="options-grid">
              <button class="option-btn" id="save_project_btn">
                💾 Save Project
                <span class="option-hint">Save configuration for later</span>
              </button>
              <button class="option-btn" id="share_results_btn">
                🔗 Share Results
                <span class="option-hint">Generate shareable link</span>
              </button>
              <button class="option-btn" id="print_report_btn">
                🖨️ Print Report
                <span class="option-hint">PDF optimization report</span>
              </button>
            </div>
          </div>

          <!-- Actions -->
          <div class="saving-actions">
            <button type="button" id="start_new_btn" class="btn-secondary btn-large">
              🔄 Start New Optimization
            </button>
            <button type="button" id="back_to_listening_btn" class="btn-outline">
              Back to Listening
            </button>
          </div>

          <!-- Tips -->
          <div class="saving-tips">
            <h4>💡 Next Steps</h4>
            <ul>
              <li>Load the exported EQ file into your audio software or hardware</li>
              <li>Fine-tune the EQ manually if needed (adjust gain/Q of individual filters)</li>
              <li>Test with different music genres to ensure consistent results</li>
              <li>Save your original settings before applying the new EQ</li>
              <li>Re-run optimization if you change speakers or room setup</li>
            </ul>
          </div>
        </div>
      </div>
    `;
  }

  /**
   * Attach event listeners
   */
  private attachEventListeners(): void {
    // Export buttons
    const exportBtns = this.container.querySelectorAll(
      ".export-btn[data-format]",
    );
    exportBtns.forEach((btn) => {
      btn.addEventListener("click", () => {
        const format = (btn as HTMLElement).dataset.format as ExportFormat;
        this.handleExport(format);
      });
    });

    // Download all button
    const downloadAllBtn = this.container.querySelector("#download_all_btn");
    if (downloadAllBtn) {
      downloadAllBtn.addEventListener("click", () => this.handleDownloadAll());
    }

    // Additional options
    const saveProjectBtn = this.container.querySelector("#save_project_btn");
    if (saveProjectBtn) {
      saveProjectBtn.addEventListener("click", () => this.handleSaveProject());
    }

    const shareResultsBtn = this.container.querySelector("#share_results_btn");
    if (shareResultsBtn) {
      shareResultsBtn.addEventListener("click", () =>
        this.handleShareResults(),
      );
    }

    const printReportBtn = this.container.querySelector("#print_report_btn");
    if (printReportBtn) {
      printReportBtn.addEventListener("click", () => this.handlePrintReport());
    }

    // Start new button
    const startNewBtn = this.container.querySelector("#start_new_btn");
    if (startNewBtn) {
      startNewBtn.addEventListener("click", () => this.handleStartNew());
    }

    // Back button
    const backBtn = this.container.querySelector("#back_to_listening_btn");
    if (backBtn) {
      backBtn.addEventListener("click", () => {
        if ((window as any).demo?.navigator) {
          (window as any).demo.navigator.goToStep(4);
        }
      });
    }
  }

  /**
   * Handle export for a specific format
   */
  private handleExport(format: ExportFormat): void {
    console.log(`📥 Exporting ${format} format...`);

    // Mark as exported
    this.exportedFormats.add(format);

    // Update UI
    const card = this.container.querySelector(
      `.export-card[data-format="${format}"]`,
    );
    if (card) {
      const btn = card.querySelector(".export-btn") as HTMLButtonElement;
      const status = card.querySelector(".export-status") as HTMLElement;

      if (btn && status) {
        btn.style.display = "none";
        status.style.display = "inline-block";
        card.classList.add("exported");
      }
    }

    // Mock download (in real app, would trigger actual download)
    this.mockDownload(`eq_optimized.${this.getFileExtension(format)}`);
  }

  /**
   * Handle download all formats
   */
  private handleDownloadAll(): void {
    console.log("📦 Downloading all formats...");

    // Export all formats
    const formats: ExportFormat[] = [
      "apo",
      "aupreset",
      "rme-channel",
      "rme-room",
      "csv",
    ];
    formats.forEach((format) => {
      if (!this.exportedFormats.has(format)) {
        this.handleExport(format);
      }
    });

    // Mock ZIP download
    setTimeout(() => {
      this.mockDownload("eq_optimized_all.zip");
      alert("✅ All formats exported successfully!");
    }, 500);
  }

  /**
   * Handle save project
   */
  private handleSaveProject(): void {
    console.log("💾 Saving project...");
    this.mockDownload("eq_project.json");
    alert("✅ Project saved! You can load it later to continue working.");
  }

  /**
   * Handle share results
   */
  private handleShareResults(): void {
    console.log("🔗 Generating share link...");
    const mockLink = "https://autoeq.app/share/abc123def456";

    // Mock copy to clipboard
    setTimeout(() => {
      alert(
        `🔗 Share link copied to clipboard!\n\n${mockLink}\n\nAnyone with this link can view your optimization results.`,
      );
    }, 300);
  }

  /**
   * Handle print report
   */
  private handlePrintReport(): void {
    console.log("🖨️ Generating PDF report...");
    setTimeout(() => {
      this.mockDownload("eq_optimization_report.pdf");
    }, 500);
  }

  /**
   * Handle start new optimization
   */
  private handleStartNew(): void {
    if (
      confirm(
        "Start a new optimization?\n\nThis will reset the current workflow.",
      )
    ) {
      console.log("🔄 Starting new optimization...");

      if (this.config.onStartNew) {
        this.config.onStartNew();
      }
    }
  }

  /**
   * Get file extension for format
   */
  private getFileExtension(format: ExportFormat): string {
    const extensions: Record<ExportFormat, string> = {
      apo: "txt",
      aupreset: "aupreset",
      "rme-channel": "txt",
      "rme-room": "txt",
      csv: "csv",
    };
    return extensions[format];
  }

  /**
   * Mock file download
   */
  private mockDownload(filename: string): void {
    console.log(`⬇️ Downloading: ${filename}`);
    // In real app, would create blob and trigger download
    // For demo, just show console message
  }

  /**
   * Get exported formats
   */
  public getExportedFormats(): ExportFormat[] {
    return Array.from(this.exportedFormats);
  }

  /**
   * Update configuration
   */
  public updateConfig(config: Partial<SavingStepConfig>): void {
    this.config = { ...this.config, ...config };
  }

  /**
   * Refresh the component
   */
  public refresh(): void {
    this.render();
    this.attachEventListeners();
  }

  /**
   * Destroy the component
   */
  public destroy(): void {
    this.container.innerHTML = "";
    this.container.classList.remove("saving-step");
  }
}
