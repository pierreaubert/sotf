// Use Case Selector Component
// Large icon-based selector for choosing the optimization workflow

export type UseCase =
  | "file"
  | "speaker"
  | "headphone"
  | "capture"
  | "play-music";

export interface UseCaseOption {
  id: UseCase;
  title: string;
  description: string;
  icon: string; // SVG path data
}

export interface UseCaseSelectorConfig {
  onSelect?: (useCase: UseCase) => void;
  selectedUseCase?: UseCase;
}

export class UseCaseSelector {
  private container: HTMLElement;
  private config: UseCaseSelectorConfig;
  private selectedUseCase: UseCase | null = null;

  private readonly useCases: UseCaseOption[] = [
    {
      id: "play-music",
      title: "Play Music",
      description:
        "Jump straight to the audio player to test and listen to music",
      icon: `<circle cx="12" cy="12" r="10"></circle>
             <polygon points="10 8 16 12 10 16 10 8"></polygon>`,
    },
    {
      id: "speaker",
      title: "Speaker",
      description:
        "Optimize speakers using online measurements from spinorama.org",
      icon: `<rect width="16" height="20" x="4" y="2" rx="2"></rect>
             <path d="M12 6h.01"></path>
             <circle cx="12" cy="14" r="4"></circle>
             <path d="M12 14h.01"></path>`,
    },
    {
      id: "headphone",
      title: "Headphone",
      description: "Optimize headphones using online measurements",
      icon: `<path d="M3 18v-6a9 9 0 0 1 18 0v6"></path>
             <path d="M21 19a2 2 0 0 1-2 2h-1a2 2 0 0 1-2-2v-3a2 2 0 0 1 2-2h3zM3 19a2 2 0 0 0 2 2h1a2 2 0 0 0 2-2v-3a2 2 0 0 0-2-2H3z"></path>`,
    },
    {
      id: "capture",
      title: "Microphone Capture",
      description:
        "Measure your device live using a microphone and test signals",
      icon: `<path d="M12 1a3 3 0 0 0-3 3v8a3 3 0 0 0 6 0V4a3 3 0 0 0-3-3z"></path>
             <path d="M19 10v2a7 7 0 0 1-14 0v-2"></path>
             <line x1="12" y1="19" x2="12" y2="23"></line>
             <line x1="8" y1="23" x2="16" y2="23"></line>`,
    },
    {
      id: "file",
      title: "CSV Files",
      description: "Import custom measurement data from CSV files",
      icon: `<path d="M13 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z"></path>
             <polyline points="13 2 13 9 20 9"></polyline>`,
    },
  ];

  constructor(container: HTMLElement, config: UseCaseSelectorConfig = {}) {
    this.container = container;
    this.config = config;
    this.selectedUseCase = config.selectedUseCase || null;
    this.render();
  }

  /**
   * Render the use case selector
   */
  private render(): void {
    this.container.classList.add("use-case-selector");
    this.container.innerHTML = this.generateHTML();
    this.attachEventListeners();
  }

  /**
   * Generate HTML for the selector
   */
  private generateHTML(): string {
    return `
      <section class="section">
        <div class="container">
          <div class="has-text-centered mb-6">
            <h1 class="title is-2">Choose Your Use Case</h1>
            <p class="subtitle is-5">
              Select the type of device or measurement method for EQ optimization
            </p>
          </div>

          <div class="columns is-multiline is-centered">
            ${this.useCases.map((useCase) => this.generateCardHTML(useCase)).join("")}
          </div>

          <div class="notification is-info is-light mt-6">
            <p>
              <strong>Not sure which to choose?</strong>
              Start with <strong>Speaker</strong> or <strong>Headphone</strong>
              to browse thousands of professional measurements.
            </p>
          </div>
        </div>
      </section>
    `;
  }

  /**
   * Generate HTML for a single use case card
   */
  private generateCardHTML(useCase: UseCaseOption): string {
    const isSelected = this.selectedUseCase === useCase.id;
    const selectedClass = isSelected ? "is-primary" : "";

    return `
      <div class="column is-one-third-desktop is-half-tablet">
        <div class="box use-case-card ${selectedClass}" data-use-case="${useCase.id}" tabindex="0" role="button">
          <div class="use-case-card-icon">
            <svg
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
            >
              ${useCase.icon}
            </svg>
          </div>
          <h3 class="title is-5 mt-4">${useCase.title}</h3>
          <p class="content">${useCase.description}</p>
          ${isSelected ? '<span class="tag is-success is-light">Selected</span>' : ""}
        </div>
      </div>
    `;
  }

  /**
   * Attach event listeners to cards
   */
  private attachEventListeners(): void {
    const cards = this.container.querySelectorAll(".use-case-card");

    cards.forEach((card) => {
      card.addEventListener("click", () => {
        const useCaseId = (card as HTMLElement).dataset.useCase as UseCase;
        this.selectUseCase(useCaseId);
      });

      // Add keyboard support
      card.addEventListener("keypress", (e) => {
        if (
          (e as KeyboardEvent).key === "Enter" ||
          (e as KeyboardEvent).key === " "
        ) {
          const useCaseId = (card as HTMLElement).dataset.useCase as UseCase;
          this.selectUseCase(useCaseId);
        }
      });
    });
  }

  /**
   * Select a use case
   */
  public selectUseCase(useCase: UseCase): void {
    // Update selection
    this.selectedUseCase = useCase;

    // Update UI
    const cards = this.container.querySelectorAll(".use-case-card");
    cards.forEach((card) => {
      const cardUseCase = (card as HTMLElement).dataset.useCase;
      if (cardUseCase === useCase) {
        card.classList.add("is-primary");
        // Add badge if not present
        if (!card.querySelector(".tag")) {
          const badge = document.createElement("span");
          badge.className = "tag is-success is-light";
          badge.textContent = "Selected";
          card.appendChild(badge);
        }
      } else {
        card.classList.remove("is-primary");
        // Remove badge if present
        const badge = card.querySelector(".tag");
        if (badge) badge.remove();
      }
    });

    // Call callback
    if (this.config.onSelect) {
      this.config.onSelect(useCase);
    }
  }

  /**
   * Get the currently selected use case
   */
  public getSelectedUseCase(): UseCase | null {
    return this.selectedUseCase;
  }

  /**
   * Set the selected use case programmatically
   */
  public setSelectedUseCase(useCase: UseCase): void {
    this.selectUseCase(useCase);
  }

  /**
   * Clear the selection
   */
  public clearSelection(): void {
    this.selectedUseCase = null;

    const cards = this.container.querySelectorAll(".use-case-card");
    cards.forEach((card) => {
      card.classList.remove("is-primary");
      const badge = card.querySelector(".tag");
      if (badge) badge.remove();
    });
  }

  /**
   * Update configuration
   */
  public updateConfig(config: Partial<UseCaseSelectorConfig>): void {
    this.config = { ...this.config, ...config };
  }

  /**
   * Refresh the component
   */
  public refresh(): void {
    this.render();
  }

  /**
   * Destroy the component
   */
  public destroy(): void {
    this.container.innerHTML = "";
    this.container.classList.remove("use-case-selector");
  }
}
