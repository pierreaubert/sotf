// Step Navigator Component
// Provides navigation between different steps of the application workflow

export interface Step {
  id: number;
  label: string;
  shortLabel?: string; // For mobile/small screens
  enabled: boolean;
}

export interface StepNavigatorConfig {
  steps: Step[];
  currentStep: number;
  onStepChange?: (stepId: number) => void;
  onPrevious?: () => void;
  onNext?: () => void;
}

export class StepNavigator {
  private config: StepNavigatorConfig;
  private container: HTMLElement;
  private navElement: HTMLElement | null = null;

  constructor(container: HTMLElement, config: StepNavigatorConfig) {
    this.container = container;
    this.config = config;
    this.render();
  }

  /**
   * Render the step navigator
   */
  private render(): void {
    this.navElement = document.createElement("div");
    this.navElement.className = "step-navigator";
    this.navElement.innerHTML = this.generateHTML();
    this.container.appendChild(this.navElement);
    this.attachEventListeners();
  }

  /**
   * Generate the HTML for the navigator
   */
  private generateHTML(): string {
    const stepsHTML = this.config.steps
      .map((step) => this.generateStepHTML(step))
      .join("");

    return `
      <div class="step-nav-container">
        <button class="step-nav-btn step-nav-prev" data-action="prev" aria-label="Previous step">
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <polyline points="15 18 9 12 15 6"></polyline>
          </svg>
        </button>

        <div class="step-nav-steps">
          ${stepsHTML}
        </div>

        <button class="step-nav-btn step-nav-next" data-action="next" aria-label="Next step">
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <polyline points="9 18 15 12 9 6"></polyline>
          </svg>
        </button>
      </div>
    `;
  }

  /**
   * Generate HTML for a single step
   */
  private generateStepHTML(step: Step): string {
    const isCurrent = step.id === this.config.currentStep;
    const isCompleted = step.id < this.config.currentStep;
    const isEnabled = step.enabled;

    const classes = [
      "step-nav-item",
      isCurrent ? "active" : "",
      isCompleted ? "completed" : "",
      !isEnabled ? "disabled" : "",
    ]
      .filter(Boolean)
      .join(" ");

    return `
      <div class="${classes}" data-step="${step.id}">
        <div class="step-nav-number">${step.id}</div>
        <div class="step-nav-label">
          <span class="step-nav-label-full">${step.label}</span>
          ${step.shortLabel ? `<span class="step-nav-label-short">${step.shortLabel}</span>` : ""}
        </div>
      </div>
    `;
  }

  /**
   * Attach event listeners to navigation buttons and steps
   */
  private attachEventListeners(): void {
    if (!this.navElement) return;

    // Previous button
    const prevBtn = this.navElement.querySelector(".step-nav-prev");
    if (prevBtn) {
      prevBtn.addEventListener("click", () => this.handlePrevious());
    }

    // Next button
    const nextBtn = this.navElement.querySelector(".step-nav-next");
    if (nextBtn) {
      nextBtn.addEventListener("click", () => this.handleNext());
    }

    // Step items
    const stepItems = this.navElement.querySelectorAll(".step-nav-item");
    stepItems.forEach((item) => {
      item.addEventListener("click", (e) => {
        const stepId = parseInt(
          (e.currentTarget as HTMLElement).dataset.step || "0",
          10,
        );
        this.handleStepClick(stepId);
      });
    });
  }

  /**
   * Handle previous button click
   */
  private handlePrevious(): void {
    if (this.config.currentStep > 1) {
      const prevStep = this.config.currentStep - 1;
      if (this.config.onPrevious) {
        this.config.onPrevious();
      }
      this.goToStep(prevStep);
    }
  }

  /**
   * Handle next button click
   */
  private handleNext(): void {
    const maxStep = this.config.steps.length;
    if (this.config.currentStep < maxStep) {
      const nextStep = this.config.currentStep + 1;
      if (this.config.onNext) {
        this.config.onNext();
      }
      this.goToStep(nextStep);
    }
  }

  /**
   * Handle step item click
   */
  private handleStepClick(stepId: number): void {
    const step = this.config.steps.find((s) => s.id === stepId);
    if (step && step.enabled) {
      this.goToStep(stepId);
    }
  }

  /**
   * Navigate to a specific step
   */
  public goToStep(stepId: number): void {
    const step = this.config.steps.find((s) => s.id === stepId);
    if (!step || !step.enabled) return;

    this.config.currentStep = stepId;
    this.updateUI();

    if (this.config.onStepChange) {
      this.config.onStepChange(stepId);
    }
  }

  /**
   * Update the UI to reflect the current step
   */
  private updateUI(): void {
    if (!this.navElement) return;

    const stepItems = this.navElement.querySelectorAll(".step-nav-item");
    stepItems.forEach((item) => {
      const stepId = parseInt((item as HTMLElement).dataset.step || "0", 10);
      const isCurrent = stepId === this.config.currentStep;
      const isCompleted = stepId < this.config.currentStep;

      item.classList.toggle("active", isCurrent);
      item.classList.toggle("completed", isCompleted);
    });

    // Update button states
    const prevBtn = this.navElement.querySelector(".step-nav-prev");
    const nextBtn = this.navElement.querySelector(".step-nav-next");

    if (prevBtn) {
      prevBtn.classList.toggle("disabled", this.config.currentStep === 1);
    }

    if (nextBtn) {
      nextBtn.classList.toggle(
        "disabled",
        this.config.currentStep === this.config.steps.length,
      );
    }
  }

  /**
   * Update step configuration
   */
  public updateSteps(steps: Step[]): void {
    this.config.steps = steps;
    this.refresh();
  }

  /**
   * Enable or disable a specific step
   */
  public setStepEnabled(stepId: number, enabled: boolean): void {
    const step = this.config.steps.find((s) => s.id === stepId);
    if (step) {
      step.enabled = enabled;
      this.updateUI();
    }
  }

  /**
   * Get the current step ID
   */
  public getCurrentStep(): number {
    return this.config.currentStep;
  }

  /**
   * Refresh the navigator (re-render)
   */
  public refresh(): void {
    if (this.navElement) {
      this.navElement.innerHTML = this.generateHTML();
      this.attachEventListeners();
    }
  }

  /**
   * Destroy the navigator and clean up
   */
  public destroy(): void {
    if (this.navElement) {
      this.navElement.remove();
      this.navElement = null;
    }
  }
}
