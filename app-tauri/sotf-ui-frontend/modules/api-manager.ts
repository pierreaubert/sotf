// API management and data fetching functionality

import { invoke } from "@tauri-apps/api/core";
import { open as openDialog } from "@tauri-apps/plugin-dialog";

export interface SpeakerData {
  name: string;
  versions: string[];
  measurements: { [version: string]: string[] };
}

export class APIManager {
  // API data caching
  private speakers: string[] = [];
  private selectedSpeaker: string = "";
  private selectedVersion: string = "";
  private speakerData: { [key: string]: SpeakerData } = {};

  // Autocomplete data
  private autocompleteData: string[] = [];

  constructor() {
    this.loadSpeakers();
  }

  /**
   * Enables a select element with cross-browser compatibility
   * Addresses issues with Chrome on Linux where disabled selects don't re-enable properly
   */
  private enableSelectElement(selectElement: HTMLSelectElement): void {
    try {
      // Method 1: Remove disabled attribute and property
      selectElement.removeAttribute("disabled");
      selectElement.disabled = false;

      // Method 2: Remove any CSS-based disabling
      selectElement.style.pointerEvents = "";
      selectElement.style.opacity = "";
      selectElement.classList.remove("disabled");

      // Method 3: Force complete DOM reflow with multiple techniques
      const originalDisplay = selectElement.style.display;
      selectElement.style.display = "none";
      void selectElement.offsetHeight; // Force reflow
      selectElement.style.display = originalDisplay || "";

      // Method 4: Force style recalculation by changing and reverting a style
      const originalPosition = selectElement.style.position;
      selectElement.style.position = "relative";
      void selectElement.offsetWidth; // Force style recalculation
      selectElement.style.position = originalPosition || "";

      // Method 5: Use requestAnimationFrame to ensure DOM updates are processed
      requestAnimationFrame(() => {
        selectElement.removeAttribute("disabled");
        selectElement.disabled = false;

        // Method 6: Trigger synthetic events to force browser recognition
        const changeEvent = new Event("change", { bubbles: true });
        const focusEvent = new Event("focus", { bubbles: true });

        setTimeout(() => {
          if (selectElement.disabled === false) {
            selectElement.focus();
            selectElement.dispatchEvent(focusEvent);
            if (typeof selectElement.blur === "function") {
              selectElement.blur();
            }
            selectElement.dispatchEvent(changeEvent);
          }

          // Final verification - if still disabled, try nuclear option
          if (selectElement.disabled === true) {
            console.warn(
              `Select element ${selectElement.id} is still disabled after all attempts`,
            );
            this.nuclearEnableSelect(selectElement);
          }
        }, 20);
      });
    } catch (error) {
      console.error("Error enabling select element:", error);
      // Fallback: just set disabled to false
      selectElement.disabled = false;
    }
  }

  /**
   * Nuclear option: completely replace the select element if it's still disabled
   * This is a last resort for Chrome on Linux
   */
  private nuclearEnableSelect(selectElement: HTMLSelectElement): void {
    try {
      const parent = selectElement.parentElement;
      if (!parent) return;

      // Clone the element without the disabled attribute
      const newSelect = document.createElement("select");
      newSelect.id = selectElement.id;
      newSelect.name = selectElement.name;
      newSelect.className = selectElement.className;

      // Copy all options
      Array.from(selectElement.options).forEach((option) => {
        const newOption = document.createElement("option");
        newOption.value = option.value;
        newOption.textContent = option.textContent;
        newOption.selected = option.selected;
        newSelect.appendChild(newOption);
      });

      // Ensure it's not disabled
      newSelect.disabled = false;
      newSelect.removeAttribute("disabled");

      // Replace the old element
      parent.replaceChild(newSelect, selectElement);
    } catch (error) {
      console.error("Nuclear enable failed:", error);
    }
  }

  async loadSpeakers(): Promise<void> {
    try {
      const speakers = (await invoke("get_speakers")) as string[];

      // Ensure we have a valid array
      if (Array.isArray(speakers)) {
        this.speakers = speakers;

        // Update speaker dropdown
        this.updateSpeakerDropdown();

        // Load autocomplete data
        this.autocompleteData = [...speakers];
      } else {
        throw new Error("Invalid response format: expected array");
      }
    } catch (error) {
      console.error("Failed to load speakers:", error);
      // No fallback - keep empty list
      this.speakers = [];
      this.autocompleteData = [];
    }
  }

  async loadSpeakerVersions(speaker: string): Promise<string[]> {
    try {
      // Try the backend call with proper parameter structure
      const result = await invoke("get_speaker_versions", {
        speaker: speaker,
      });

      // Handle different response formats
      let versions: string[];
      if (Array.isArray(result)) {
        versions = result as string[];
      } else if (result && typeof result === "object" && "versions" in result) {
        versions = (result as Record<string, unknown>).versions as string[];
      } else {
        throw new Error("Invalid response format from backend");
      }

      // Cache the data
      if (!this.speakerData[speaker]) {
        this.speakerData[speaker] = {
          name: speaker,
          versions: versions,
          measurements: {},
        };
      } else {
        this.speakerData[speaker].versions = versions;
      }

      return versions;
    } catch (error) {
      console.warn("Backend speaker versions not available:", error);

      // Return empty array instead of fallback data
      const emptyVersions: string[] = [];

      // Cache the empty result
      if (!this.speakerData[speaker]) {
        this.speakerData[speaker] = {
          name: speaker,
          versions: emptyVersions,
          measurements: {},
        };
      } else {
        this.speakerData[speaker].versions = emptyVersions;
      }

      return emptyVersions;
    }
  }

  async loadSpeakerMeasurements(
    speaker: string,
    version: string,
  ): Promise<string[]> {
    try {
      // Try the backend call with proper parameter structure
      const result = await invoke("get_speaker_measurements", {
        speaker: speaker,
        version: version,
      });

      // Handle different response formats
      let measurements: string[];
      if (Array.isArray(result)) {
        measurements = result as string[];
      } else if (
        result &&
        typeof result === "object" &&
        "measurements" in result
      ) {
        measurements = (result as Record<string, unknown>)
          .measurements as string[];
      } else {
        throw new Error("Invalid response format from backend");
      }

      // Cache the data
      if (!this.speakerData[speaker]) {
        this.speakerData[speaker] = {
          name: speaker,
          versions: [],
          measurements: {},
        };
      }
      this.speakerData[speaker].measurements[version] = measurements;

      return measurements;
    } catch (error) {
      console.warn("Backend speaker measurements not available:", error);

      // Return empty array instead of fallback data
      const emptyMeasurements: string[] = [];

      // Cache the empty result
      if (!this.speakerData[speaker]) {
        this.speakerData[speaker] = {
          name: speaker,
          versions: [],
          measurements: {},
        };
      }
      this.speakerData[speaker].measurements[version] = emptyMeasurements;
      return emptyMeasurements;
    }
  }

  private updateSpeakerDropdown(): void {
    const speakerSelect = document.getElementById(
      "speaker",
    ) as HTMLSelectElement;
    if (!speakerSelect) return;

    // Clear existing options
    speakerSelect.innerHTML = '<option value="">Select a speaker...</option>';

    // Add speaker options
    this.speakers.forEach((speaker) => {
      const option = document.createElement("option");
      option.value = speaker;
      option.textContent = speaker;
      speakerSelect.appendChild(option);
    });
  }

  async handleSpeakerChange(speaker: string): Promise<void> {
    this.selectedSpeaker = speaker;
    this.selectedVersion = "";

    const versionSelect = document.getElementById(
      "version",
    ) as HTMLSelectElement;
    const measurementSelect = document.getElementById(
      "measurement",
    ) as HTMLSelectElement;

    if (!versionSelect || !measurementSelect) return;

    // Clear dependent dropdowns
    versionSelect.innerHTML = '<option value="">Select a version...</option>';
    measurementSelect.innerHTML =
      '<option value="">Select a measurement...</option>';

    if (!speaker) {
      // Disable dropdowns if no speaker selected
      versionSelect.disabled = true;
      measurementSelect.disabled = true;
      return;
    }

    // Set loss function to "speaker-flat" when a speaker is selected
    const lossSelect = document.getElementById("loss") as HTMLSelectElement;
    if (lossSelect) {
      lossSelect.value = "speaker-flat";
    }

    try {
      const versions = await this.loadSpeakerVersions(speaker);

      if (versions.length > 0) {
        versions.forEach((version) => {
          const option = document.createElement("option");
          option.value = version;
          option.textContent = version;
          versionSelect.appendChild(option);
        });

        // Enable version dropdown with cross-browser compatibility
        this.enableSelectElement(versionSelect);

        // Automatically select the first version
        versionSelect.value = versions[0];
        this.selectedVersion = versions[0];

        // Trigger version change to load measurements
        await this.handleVersionChange(versions[0]);
      } else {
        // No versions available, keep dropdown disabled
        versionSelect.disabled = true;
      }
    } catch (error) {
      console.error("Error loading versions for speaker:", speaker, error);
      // Keep version dropdown disabled on error
      versionSelect.disabled = true;
    }
  }

  async handleVersionChange(version: string): Promise<void> {
    this.selectedVersion = version;

    const measurementSelect = document.getElementById(
      "measurement",
    ) as HTMLSelectElement;
    if (!measurementSelect) return;

    measurementSelect.innerHTML =
      '<option value="">Select a measurement...</option>';

    if (!version || !this.selectedSpeaker) {
      measurementSelect.disabled = true;
      return;
    }

    try {
      const measurements = await this.loadSpeakerMeasurements(
        this.selectedSpeaker,
        version,
      );

      if (measurements.length > 0) {
        measurements.forEach((measurement) => {
          const option = document.createElement("option");
          option.value = measurement;
          option.textContent = measurement;
          measurementSelect.appendChild(option);
        });

        this.enableSelectElement(measurementSelect);
        measurementSelect.value = measurements[0];
      } else {
        measurementSelect.disabled = true;
        console.log("No measurements available for version:", version);
      }
    } catch (error) {
      console.error("Error loading measurements for version:", version, error);
      measurementSelect.disabled = true;
    }
  }

  async selectCurveFile(): Promise<string | null> {
    try {
      const input = document.getElementById("curve_path") as HTMLInputElement;
      if (!input) {
        console.error("Curve path input element not found");
        return null;
      }

      // Enhanced dialog options for better compatibility
      const result = await openDialog({
        multiple: false,
        directory: false,
        filters: [
          {
            name: "CSV Files",
            extensions: ["csv"],
          },
          {
            name: "All Files",
            extensions: ["*"],
          },
        ],
        title: "Select Input CSV File",
      });

      if (result && typeof result === "string") {
        input.value = result;
        input.dispatchEvent(new Event("input", { bubbles: true }));
        input.dispatchEvent(new Event("change", { bubbles: true }));
        this.showFileSelectionSuccess("curve-path", result);
        return result;
      } else if (result === null) {
      } else if (Array.isArray(result) && result.length > 0) {
        // Handle array result (shouldn't happen with multiple: false, but just in case)
        const filePath = result[0];
        input.value = filePath;
        input.dispatchEvent(new Event("input", { bubbles: true }));
        input.dispatchEvent(new Event("change", { bubbles: true }));
        this.showFileSelectionSuccess("curve-path", filePath);
        return filePath;
      }

      return null;
    } catch (error) {
      console.error("Error selecting curve file:", error);
      this.showFileDialogError(error);
      // Fallback: try to trigger a native file input
      return this.fallbackFileDialog("curve-path");
    }
  }

  async selectTargetFile(): Promise<string | null> {
    try {
      const input = document.getElementById("target_path") as HTMLInputElement;
      if (!input) {
        console.error("Target path input element not found");
        return null;
      }

      // Enhanced dialog options for better compatibility
      const result = await openDialog({
        multiple: false,
        directory: false,
        filters: [
          {
            name: "CSV Files",
            extensions: ["csv"],
          },
          {
            name: "All Files",
            extensions: ["*"],
          },
        ],
        title: "Select Target CSV File (Optional)",
      });

      if (result && typeof result === "string") {
        input.value = result;
        input.dispatchEvent(new Event("input", { bubbles: true }));
        input.dispatchEvent(new Event("change", { bubbles: true }));
        this.showFileSelectionSuccess("target-path", result);
        return result;
      } else if (Array.isArray(result) && result.length > 0) {
        // Handle array result (shouldn't happen with multiple: false, but just in case)
        const filePath = result[0];
        input.value = filePath;
        input.dispatchEvent(new Event("input", { bubbles: true }));
        input.dispatchEvent(new Event("change", { bubbles: true }));
        this.showFileSelectionSuccess("target-path", filePath);
        return filePath;
      }

      return null;
    } catch (error) {
      console.error("Error selecting target file:", error);
      this.showFileDialogError(error);
      // Fallback: try to trigger a native file input
      return this.fallbackFileDialog("target-path");
    }
  }

  async selectHeadphoneCurveFile(): Promise<string | null> {
    try {
      const input = document.getElementById(
        "headphone_curve_path",
      ) as HTMLInputElement;
      if (!input) {
        console.error("Headphone curve path input element not found");
        return null;
      }

      // Enhanced dialog options for better compatibility
      const result = await openDialog({
        multiple: false,
        directory: false,
        filters: [
          {
            name: "CSV Files",
            extensions: ["csv"],
          },
          {
            name: "All Files",
            extensions: ["*"],
          },
        ],
        title: "Select Headphone Curve CSV File",
      });

      if (result && typeof result === "string") {
        input.value = result;
        input.dispatchEvent(new Event("input", { bubbles: true }));
        input.dispatchEvent(new Event("change", { bubbles: true }));
        this.showFileSelectionSuccess("headphone_curve_path", result);
        return result;
      } else if (Array.isArray(result) && result.length > 0) {
        // Handle array result (shouldn't happen with multiple: false, but just in case)
        const filePath = result[0];
        input.value = filePath;
        input.dispatchEvent(new Event("input", { bubbles: true }));
        input.dispatchEvent(new Event("change", { bubbles: true }));
        this.showFileSelectionSuccess("headphone_curve_path", filePath);
        return filePath;
      }

      return null;
    } catch (error) {
      console.error("Error selecting headphone curve file:", error);
      this.showFileDialogError(error);
      // Fallback: try to trigger a native file input
      return this.fallbackFileDialog("headphone_curve_path");
    }
  }

  setupAutocomplete(): void {
    const speakerInput = document.getElementById("speaker") as HTMLInputElement;
    if (!speakerInput) return;

    let autocompleteContainer: HTMLElement | null = null;

    const showAutocomplete = (suggestions: string[]) => {
      this.hideAutocomplete();

      if (suggestions.length === 0) return;

      autocompleteContainer = document.createElement("div");
      autocompleteContainer.className = "autocomplete-suggestions";
      // Check if dark mode is active
      const isDarkMode =
        document.body.classList.contains("dark-mode") ||
        document.documentElement.classList.contains("dark-mode") ||
        window.matchMedia("(prefers-color-scheme: dark)").matches;

      autocompleteContainer.style.cssText = `
        position: absolute;
        top: 100%;
        left: 0;
        right: 0;
        background: ${isDarkMode ? "#2d3748" : "white"};
        color: ${isDarkMode ? "#e2e8f0" : "#333"};
        border: 1px solid ${isDarkMode ? "#4a5568" : "#ccc"};
        border-top: none;
        max-height: 200px;
        overflow-y: auto;
        z-index: 10000;
        box-shadow: 0 4px 8px rgba(0,0,0,${isDarkMode ? "0.3" : "0.15"});
        border-radius: 0 0 4px 4px;
      `;

      suggestions.forEach((suggestion) => {
        const item = document.createElement("div");
        item.className = "autocomplete-item";
        item.textContent = suggestion;
        item.style.cssText = `
          padding: 8px 12px;
          cursor: pointer;
          border-bottom: 1px solid ${isDarkMode ? "#4a5568" : "#eee"};
          color: ${isDarkMode ? "#e2e8f0" : "#333"};
        `;

        item.addEventListener("mouseenter", () => {
          item.style.backgroundColor = isDarkMode ? "#4a5568" : "#f0f0f0";
        });

        item.addEventListener("mouseleave", () => {
          item.style.backgroundColor = isDarkMode ? "#2d3748" : "white";
        });

        item.addEventListener("click", (e) => {
          e.preventDefault();
          e.stopPropagation();
          speakerInput.value = suggestion;
          this.handleSpeakerChange(suggestion);
          this.hideAutocomplete();
          speakerInput.focus();
        });

        autocompleteContainer!.appendChild(item);
      });

      // Find the correct container - look for the param-item or similar container
      let inputContainer = speakerInput.parentElement;

      // Look for a suitable container (param-item, form-group, etc.)
      if (
        !inputContainer ||
        (!inputContainer.classList.contains("param-item") &&
          !inputContainer.classList.contains("autocomplete-container") &&
          !inputContainer.classList.contains("form-group"))
      ) {
        inputContainer =
          speakerInput.closest(".param-item") ||
          speakerInput.closest(".form-group") ||
          speakerInput.closest(".autocomplete-container");
      }

      if (inputContainer) {
        inputContainer.style.position = "relative";
        inputContainer.appendChild(autocompleteContainer);
      } else {
        console.warn(
          "Could not find suitable container, appending to document body",
        );
        // Fallback: append to body with fixed positioning
        const rect = speakerInput.getBoundingClientRect();
        autocompleteContainer.style.position = "fixed";
        autocompleteContainer.style.top = `${rect.bottom + window.scrollY}px`;
        autocompleteContainer.style.left = `${rect.left + window.scrollX}px`;
        autocompleteContainer.style.width = `${rect.width}px`;
        autocompleteContainer.style.zIndex = "10001"; // Higher than modal z-index
        document.body.appendChild(autocompleteContainer);
      }
    };

    const hideAutocomplete = () => {
      if (autocompleteContainer) {
        autocompleteContainer.remove();
        autocompleteContainer = null;
      }
    };

    this.hideAutocomplete = hideAutocomplete;

    speakerInput.addEventListener("input", (e) => {
      const value = (e.target as HTMLInputElement).value.toLowerCase();

      if (value.length < 2) {
        hideAutocomplete();
        return;
      }

      const suggestions = this.autocompleteData
        .filter((item) => item.toLowerCase().includes(value))
        .slice(0, 10);

      showAutocomplete(suggestions);
    });

    speakerInput.addEventListener("blur", () => {
      // Delay hiding to allow click events on suggestions
      setTimeout(hideAutocomplete, 150);
    });

    document.addEventListener("click", (e) => {
      if (
        !speakerInput.contains(e.target as Node) &&
        !autocompleteContainer?.contains(e.target as Node)
      ) {
        hideAutocomplete();
      }
    });
  }

  private hideAutocomplete: () => void = () => {};

  async loadDemoAudioList(): Promise<string[]> {
    let audioList: string[];
    try {
      // Try to get demo audio list from backend first
      audioList = (await invoke("get_demo_audio_list")) as string[];
    } catch (_error) {
      // Fallback: Use actual demo audio files from public/demo-audio/
      audioList = [
        "classical.flac",
        "country.flac",
        "edm.flac",
        "female_vocal.flac",
        "jazz.flac",
        "piano.flac",
        "rock.flac",
      ];
    }

    const demoAudioSelect = document.getElementById(
      "demo_audio_select",
    ) as HTMLSelectElement;
    if (demoAudioSelect) {
      // Clear existing options
      demoAudioSelect.innerHTML =
        '<option value="">Select demo audio...</option>';

      // Add a special option for loading from file
      const loadFromFileOption = document.createElement("option");
      loadFromFileOption.value = "load_from_file";
      loadFromFileOption.textContent = "Load from file...";
      demoAudioSelect.appendChild(loadFromFileOption);

      // Add a separator
      const separator = document.createElement("option");
      separator.disabled = true;
      separator.textContent = "──────────";
      demoAudioSelect.appendChild(separator);

      // Add audio options from the determined list
      audioList.forEach((audio) => {
        const option = document.createElement("option");
        option.value = audio.replace(".flac", ""); // Remove .flac for the value
        option.textContent = audio
          .replace(".flac", "")
          .replace(/_/g, " ")
          .replace(/\b\w/g, (l) => l.toUpperCase());
        demoAudioSelect.appendChild(option);
      });
    }

    return audioList;
  }

  async getDemoAudioUrl(audioName: string): Promise<string | null> {
    try {
      // Try to get URL from backend first
      const url = (await invoke("get_demo_audio_url", {
        audio_name: audioName,
      })) as string;
      return url;
    } catch (_error) {
      // Fallback: Use local file path
      const fileName = audioName.endsWith(".flac")
        ? audioName
        : `${audioName}.flac`;
      const localUrl = `public/demo-audio/${fileName}`;
      return localUrl;
    }
  }

  // Validation helpers
  validateOptimizationParams(formData: FormData): {
    isValid: boolean;
    errors: string[];
  } {
    const errors: string[] = [];

    // Helper function to get and validate numeric values with defaults
    const getNumericValue = (
      key: string,
      defaultValue: number,
      _min?: number,
      _max?: number,
    ): number => {
      const str = formData.get(key) as string;
      if (!str || str.trim() === "") {
        return defaultValue;
      }
      const value = parseFloat(str);
      if (isNaN(value)) {
        return defaultValue;
      }
      return value;
    };

    // Use default values if form elements don't exist (using HTML form field names)
    const numFilters = getNumericValue("num_filters", 5);
    if (numFilters < 1 || numFilters > 20) {
      errors.push("Number of filters must be between 1 and 20");
    }

    const sampleRate = getNumericValue("sample_rate", 48000);
    if (sampleRate < 8000 || sampleRate > 192000) {
      errors.push("Sample rate must be between 8000 and 192000 Hz");
    }

    const maxDb = getNumericValue("max_db", 6.0);
    const minDb = getNumericValue("min_db", -1.0);
    if (maxDb <= minDb) {
      errors.push("Max dB must be greater than Min dB");
    }

    const maxQ = getNumericValue("max_q", 10);
    const minQ = getNumericValue("min_q", 0.1);
    if (maxQ <= minQ || minQ <= 0) {
      errors.push(
        "Max Q must be greater than Min Q, and Min Q must be positive",
      );
    }

    const maxFreq = getNumericValue("max_freq", 20000);
    const minFreq = getNumericValue("min_freq", 20);
    if (maxFreq <= minFreq || minFreq <= 0) {
      errors.push(
        "Max frequency must be greater than Min frequency, and Min frequency must be positive",
      );
    }

    const inputType = formData.get("input_source") as string;
    if (inputType === "speaker") {
      const speaker = formData.get("speaker") as string;
      const version = formData.get("version") as string;
      const measurement = formData.get("measurement") as string;

      if (!speaker) errors.push("Speaker selection is required");
      if (!version) errors.push("Version selection is required");
      if (!measurement) errors.push("Measurement selection is required");
    } else if (inputType === "headphone") {
      const curvePath = formData.get("headphone_curve_path") as string;
      const target = formData.get("headphone_target") as string;

      if (!curvePath) errors.push("Headphone curve file is required");
      if (!target) errors.push("Headphone target selection is required");
    } else if (inputType === "file") {
      const curvePath = formData.get("curve_path") as string;
      const targetPath = formData.get("target_path") as string;

      if (!curvePath) errors.push("Curve file is required");
      if (!targetPath) errors.push("Target file is required");
    }

    return {
      isValid: errors.length === 0,
      errors,
    };
  }

  // Getters
  getSpeakers(): string[] {
    return [...this.speakers];
  }

  getSelectedSpeaker(): string {
    return this.selectedSpeaker;
  }

  getSelectedVersion(): string {
    return this.selectedVersion;
  }

  getSpeakerData(speaker: string): SpeakerData | null {
    return this.speakerData[speaker] || null;
  }

  getAutocompleteData(): string[] {
    return [...this.autocompleteData];
  }

  private showFileSelectionSuccess(inputId: string, filePath: string): void {
    const fileName = filePath.split("/").pop() || filePath;
    const message = `Selected file: ${fileName}`;

    // Add visual feedback to the input
    const input = document.getElementById(inputId) as HTMLInputElement;
    if (input) {
      input.style.borderColor = "#28a745"; // Green border for success
      input.title = `Selected: ${filePath}`;
      setTimeout(() => {
        input.style.borderColor = ""; // Reset border after 2 seconds
      }, 2000);
    }
  }

  private showFileDialogError(error: unknown): void {
    console.error("File dialog error details:", error);
    const errorMessage = error instanceof Error ? error.message : String(error);
    const message = `File dialog failed: ${errorMessage}. Using fallback file picker.`;
    console.warn(message);
    this.showTemporaryMessage(message, "error");
  }

  private fallbackFileDialog(inputId: string): Promise<string | null> {
    return new Promise((resolve) => {
      const input = document.getElementById(inputId) as HTMLInputElement;
      const fileInput = document.createElement("input");
      fileInput.type = "file";
      fileInput.accept = ".csv,text/csv";
      fileInput.style.display = "none";

      fileInput.onchange = (event) => {
        const file = (event.target as HTMLInputElement).files?.[0];
        if (file) {
          // In fallback mode, we can only get the filename, not the full path
          // This is a browser security limitation
          input.value = file.name; // Note: This gives filename, not full path
          input.dispatchEvent(new Event("input", { bubbles: true }));
          input.dispatchEvent(new Event("change", { bubbles: true }));
          this.showFallbackWarning(inputId, file.name);
          resolve(file.name);
        } else {
          resolve(null);
        }
      };

      document.body.appendChild(fileInput);
      fileInput.click();
      document.body.removeChild(fileInput);
    });
  }

  private showFallbackWarning(inputId: string, fileName: string): void {
    const message = `Using fallback file picker. Selected: ${fileName}. Note: Full file path not available in browser mode.`;
    console.warn(`Fallback mode for ${inputId}:`, message);
    this.showTemporaryMessage(message, "warning");

    // Add visual indication to the input
    const input = document.getElementById(inputId) as HTMLInputElement;
    if (input) {
      input.style.borderColor = "#ffc107"; // Yellow border for warning
      input.title = `Fallback mode: ${fileName} (full path not available)`;
      setTimeout(() => {
        input.style.borderColor = ""; // Reset border after 3 seconds
      }, 3000);
    }
  }

  private showTemporaryMessage(
    message: string,
    type: "error" | "warning" | "success" = "error",
  ): void {
    // Create temporary message element
    const messageDiv = document.createElement("div");
    messageDiv.textContent = message;
    messageDiv.style.cssText = `
      position: fixed;
      top: 20px;
      right: 20px;
      max-width: 400px;
      padding: 12px 16px;
      border-radius: 6px;
      font-size: 14px;
      z-index: 10000;
      box-shadow: 0 4px 12px rgba(0,0,0,0.2);
      animation: slideIn 0.3s ease-out;
      ${
        type === "error"
          ? "background-color: #dc3545; color: white;"
          : type === "warning"
            ? "background-color: #ffc107; color: black;"
            : "background-color: #28a745; color: white;"
      }
    `;

    // Add animation keyframes if not already added
    if (!document.getElementById("temp_message_styles")) {
      const style = document.createElement("style");
      style.id = "temp_message_styles";
      style.textContent = `
        @keyframes slideIn {
          from { transform: translateX(100%); opacity: 0; }
          to { transform: translateX(0); opacity: 1; }
        }
        @keyframes slideOut {
          from { transform: translateX(0); opacity: 1; }
          to { transform: translateX(100%); opacity: 0; }
        }
      `;
      document.head.appendChild(style);
    }

    document.body.appendChild(messageDiv);

    // Remove after 4 seconds
    setTimeout(() => {
      messageDiv.style.animation = "slideOut 0.3s ease-in forwards";
      setTimeout(() => {
        if (messageDiv.parentNode) {
          messageDiv.parentNode.removeChild(messageDiv);
        }
      }, 300);
    }, 4000);
  }
}
