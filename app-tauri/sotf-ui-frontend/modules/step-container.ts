// Step Container Component
// Manages content visibility and transitions between steps

export interface StepContent {
  id: number;
  element: HTMLElement;
}

export interface StepContainerConfig {
  currentStep: number;
  animationDuration?: number; // in milliseconds
  onBeforeStepChange?: (fromStep: number, toStep: number) => boolean; // return false to prevent
  onAfterStepChange?: (stepId: number) => void;
}

export class StepContainer {
  private config: StepContainerConfig;
  private container: HTMLElement;
  private contentWrapper!: HTMLElement;
  private steps: Map<number, HTMLElement> = new Map();
  private currentStepId: number;
  private isTransitioning: boolean = false;

  constructor(container: HTMLElement, config: StepContainerConfig) {
    this.container = container;
    this.config = {
      animationDuration: 300,
      ...config,
    };
    this.currentStepId = config.currentStep;
    this.initialize();
  }

  /**
   * Initialize the container
   */
  private initialize(): void {
    this.container.classList.add("step-container");

    // Create content wrapper
    this.contentWrapper = document.createElement("div");
    this.contentWrapper.className = "step-content-wrapper";
    this.container.appendChild(this.contentWrapper);

    // Collect and organize step elements
    this.collectStepElements();

    // Show initial step
    this.showStep(this.currentStepId, false);
  }

  /**
   * Collect all step elements from the container
   */
  private collectStepElements(): void {
    // Look for elements with data-step attribute
    const stepElements = this.container.querySelectorAll("[data-step]");

    stepElements.forEach((element) => {
      const stepId = parseInt((element as HTMLElement).dataset.step || "0", 10);
      if (stepId > 0) {
        const stepElement = element as HTMLElement;
        stepElement.classList.add("step-content-item");
        this.steps.set(stepId, stepElement);

        // Move to content wrapper if not already there
        if (stepElement.parentElement !== this.contentWrapper) {
          this.contentWrapper.appendChild(stepElement);
        }
      }
    });
  }

  /**
   * Register a step content element
   */
  public registerStep(stepId: number, element: HTMLElement): void {
    element.classList.add("step-content-item");
    element.dataset.step = stepId.toString();
    this.steps.set(stepId, element);

    // Add to wrapper
    this.contentWrapper.appendChild(element);

    // Hide if not current step
    if (stepId !== this.currentStepId) {
      element.classList.remove("active");
      element.style.display = "none";
    }
  }

  /**
   * Unregister a step
   */
  public unregisterStep(stepId: number): void {
    const element = this.steps.get(stepId);
    if (element) {
      element.remove();
      this.steps.delete(stepId);
    }
  }

  /**
   * Navigate to a specific step
   */
  public async goToStep(
    stepId: number,
    animate: boolean = true,
  ): Promise<boolean> {
    // Prevent navigation during transition
    if (this.isTransitioning) {
      return false;
    }

    // Check if step exists
    if (!this.steps.has(stepId)) {
      console.warn(`Step ${stepId} does not exist`);
      return false;
    }

    // Check if already on this step
    if (stepId === this.currentStepId) {
      return true;
    }

    // Call before change callback
    if (this.config.onBeforeStepChange) {
      const canProceed = this.config.onBeforeStepChange(
        this.currentStepId,
        stepId,
      );
      if (!canProceed) {
        return false;
      }
    }

    // Perform transition
    await this.showStep(stepId, animate);

    // Update current step
    this.currentStepId = stepId;

    // Call after change callback
    if (this.config.onAfterStepChange) {
      this.config.onAfterStepChange(stepId);
    }

    return true;
  }

  /**
   * Show a specific step with optional animation
   */
  private async showStep(stepId: number, animate: boolean): Promise<void> {
    const targetElement = this.steps.get(stepId);
    if (!targetElement) return;

    this.isTransitioning = true;

    if (animate && this.config.animationDuration! > 0) {
      // Hide current step with fade out
      const currentElement = this.steps.get(this.currentStepId);
      if (currentElement && currentElement !== targetElement) {
        currentElement.classList.add("step-fade-out");
        await this.wait(this.config.animationDuration! / 2);
        currentElement.classList.remove("active", "step-fade-out");
        currentElement.style.display = "none";
      }

      // Show new step with fade in
      targetElement.style.display = "flex";
      // Force reflow
      targetElement.offsetHeight;
      targetElement.classList.add("step-fade-in");
      await this.wait(this.config.animationDuration! / 2);
      targetElement.classList.add("active");
      targetElement.classList.remove("step-fade-in");
    } else {
      // No animation - instant switch
      this.steps.forEach((element, id) => {
        if (id === stepId) {
          element.style.display = "flex";
          element.classList.add("active");
        } else {
          element.style.display = "none";
          element.classList.remove("active");
        }
      });
    }

    this.isTransitioning = false;
  }

  /**
   * Get the current step ID
   */
  public getCurrentStep(): number {
    return this.currentStepId;
  }

  /**
   * Get a step element by ID
   */
  public getStepElement(stepId: number): HTMLElement | undefined {
    return this.steps.get(stepId);
  }

  /**
   * Get all registered step IDs
   */
  public getStepIds(): number[] {
    return Array.from(this.steps.keys()).sort((a, b) => a - b);
  }

  /**
   * Check if a step exists
   */
  public hasStep(stepId: number): boolean {
    return this.steps.has(stepId);
  }

  /**
   * Update configuration
   */
  public updateConfig(config: Partial<StepContainerConfig>): void {
    this.config = { ...this.config, ...config };
  }

  /**
   * Utility: Wait for a duration
   */
  private wait(ms: number): Promise<void> {
    return new Promise((resolve) => setTimeout(resolve, ms));
  }

  /**
   * Clear all step content
   */
  public clear(): void {
    this.steps.forEach((element) => {
      element.remove();
    });
    this.steps.clear();
  }

  /**
   * Destroy the container and clean up
   */
  public destroy(): void {
    this.clear();
    this.contentWrapper.remove();
    this.container.classList.remove("step-container");
  }
}
