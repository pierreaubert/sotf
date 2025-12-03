// Listening Step Component
// Audio player integration for testing the optimized EQ

export interface ListeningStepConfig {
  onContinue?: () => void;
}

export class ListeningStep {
  private container: HTMLElement;
  private config: ListeningStepConfig;

  constructor(container: HTMLElement, config: ListeningStepConfig = {}) {
    this.container = container;
    this.config = config;
    this.render();
    this.attachEventListeners();
  }

  /**
   * Render the listening step UI
   */
  private render(): void {
    this.container.classList.add("listening-step");
    this.container.innerHTML = this.generateHTML();
  }

  /**
   * Generate HTML for the listening step
   */
  private generateHTML(): string {
    return `
      <div class="step-content-wrapper">
        <div class="step-header-section">
          <h2 class="step-title">Test Your EQ</h2>
          <p class="step-description">
            Listen to your optimized EQ and compare it with the original sound. Toggle EQ on/off to hear the difference.
          </p>
        </div>

        <div class="listening-content">
          <!-- Info Section -->
          <div class="listening-info">
            <div class="info-card">
              <h3>🎧 How to Test Your EQ</h3>
              <ol class="test-instructions">
                <li>
                  <strong>Load a test track:</strong> Select from demo audio or upload your own file
                </li>
                <li>
                  <strong>Play with EQ enabled:</strong> Listen to the optimized sound
                </li>
                <li>
                  <strong>Toggle EQ off:</strong> Compare with the original unprocessed audio
                </li>
                <li>
                  <strong>Adjust volume:</strong> Ensure comfortable listening levels
                </li>
                <li>
                  <strong>Monitor spectrum:</strong> Visualize the frequency response in real-time
                </li>
              </ol>
            </div>

            <div class="info-card warning">
              <h4>⚠️ Important Notes</h4>
              <ul>
                <li>Start with moderate volume levels</li>
                <li>EQ changes the frequency balance - what sounds "better" is subjective</li>
                <li>Compare multiple tracks to evaluate the EQ across different content</li>
                <li>Pay attention to vocals, instruments, and bass response</li>
              </ul>
            </div>
          </div>

          <!-- Audio Player Container -->
          <div class="audio-player-section">
            <h3>Audio Player</h3>
            <div id="listening_audio_player" class="audio-player-container">
              <!-- AudioPlayer component will be initialized here -->
              <div class="audio-player-placeholder">
                <p>Audio player will be initialized here</p>
                <p class="placeholder-note">
                  In the full application, the AudioPlayer component would be integrated here with full EQ controls,
                  spectrum visualization, and playback controls.
                </p>
              </div>
            </div>
          </div>

          <!-- Testing Checklist -->
          <div class="testing-checklist">
            <h3>Testing Checklist</h3>
            <div class="checklist-items">
              <label class="checklist-item">
                <input type="checkbox" id="check_eq_on" />
                <span>Listened with EQ enabled</span>
              </label>
              <label class="checklist-item">
                <input type="checkbox" id="check_eq_off" />
                <span>Compared with EQ disabled</span>
              </label>
              <label class="checklist-item">
                <input type="checkbox" id="check_multiple" />
                <span>Tested with multiple tracks</span>
              </label>
              <label class="checklist-item">
                <input type="checkbox" id="check_satisfied" />
                <span>Satisfied with the results</span>
              </label>
            </div>
          </div>

          <!-- Actions -->
          <div class="listening-actions">
            <button type="button" id="continue_to_save_btn" class="btn-primary btn-large">
              Continue to Save
            </button>
            <button type="button" id="back_to_optimize_btn" class="btn-secondary">
              Back to Optimization
            </button>
          </div>

          <!-- Tips Section -->
          <div class="listening-tips">
            <h4>💡 Pro Tips</h4>
            <ul>
              <li><strong>A/B Testing:</strong> Quickly toggle EQ on/off to hear subtle differences</li>
              <li><strong>Reference Tracks:</strong> Use well-produced music you know well</li>
              <li><strong>Room Acoustics:</strong> Remember that your room affects what you hear</li>
              <li><strong>Volume Matching:</strong> EQ can change perceived loudness - adjust volume for fair comparison</li>
              <li><strong>Trust Your Ears:</strong> If it sounds better to you, it is better</li>
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
    // Continue button
    const continueBtn = this.container.querySelector(
      "#continue_to_save_btn",
    ) as HTMLButtonElement;
    if (continueBtn) {
      continueBtn.addEventListener("click", () => this.handleContinue());
    }

    // Back button
    const backBtn = this.container.querySelector(
      "#back_to_optimize_btn",
    ) as HTMLButtonElement;
    if (backBtn) {
      backBtn.addEventListener("click", () => {
        // Navigate back via window.demo if available
        if ((window as any).demo?.navigator) {
          (window as any).demo.navigator.goToStep(3);
        }
      });
    }

    // Checklist items - enable continue button when all checked
    const checkboxes = this.container.querySelectorAll(
      '.checklist-item input[type="checkbox"]',
    );
    checkboxes.forEach((checkbox) => {
      checkbox.addEventListener("change", () => this.updateContinueButton());
    });
  }

  /**
   * Update continue button state based on checklist
   */
  private updateContinueButton(): void {
    const checkboxes = this.container.querySelectorAll(
      '.checklist-item input[type="checkbox"]',
    );
    const allChecked = Array.from(checkboxes).every(
      (cb) => (cb as HTMLInputElement).checked,
    );

    const continueBtn = this.container.querySelector(
      "#continue_to_save_btn",
    ) as HTMLButtonElement;
    if (continueBtn) {
      if (allChecked) {
        continueBtn.classList.add("ready");
        continueBtn.textContent = "✓ Continue to Save";
      } else {
        continueBtn.classList.remove("ready");
        continueBtn.textContent = "Continue to Save";
      }
    }
  }

  /**
   * Handle continue button click
   */
  private handleContinue(): void {
    console.log("📦 Continuing to save step...");

    if (this.config.onContinue) {
      this.config.onContinue();
    }
  }

  /**
   * Get the audio player container element
   */
  public getAudioPlayerContainer(): HTMLElement | null {
    return this.container.querySelector("#listening_audio_player");
  }

  /**
   * Update configuration
   */
  public updateConfig(config: Partial<ListeningStepConfig>): void {
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
    this.container.classList.remove("listening-step");
  }
}
