// Audio Capture Demo - Main Entry Point
import { CaptureController } from "./capture-controller";
import { CaptureGraphRenderer } from "./capture-graph";
import { CSVExporter } from "./csv-export";
import { RoutingMatrix } from "@audio-player/audio-routing";
import { CaptureStorage, type StoredCapture } from "./capture-storage";

// Initialize when DOM is ready
document.addEventListener("DOMContentLoaded", async () => {
  console.log("Audio Capture Demo initializing...");

  // Initialize controller
  const captureController = new CaptureController();

  let currentCaptureData: {
    frequencies: number[];
    magnitudes: number[];
    phases?: number[];
    metadata: {
      timestamp: Date;
      deviceName: string;
      signalType: string;
      duration: number;
      sampleRate: number;
      outputChannel: string;
    };
  } | null = null;
  let graphRenderer: CaptureGraphRenderer | null = null;

  // Get UI Elements
  const elements = {
    inputDevice: document.getElementById("input-device") as HTMLSelectElement,
    outputDevice: document.getElementById("output-device") as HTMLSelectElement,
    signalType: document.getElementById("signal-type") as HTMLSelectElement,
    duration: document.getElementById("duration") as HTMLSelectElement,
    outputChannel: document.getElementById(
      "output-channel",
    ) as HTMLSelectElement,
    sampleRate: document.getElementById("sample-rate") as HTMLSelectElement,
    inputVolume: document.getElementById("input-volume") as HTMLInputElement,
    outputVolume: document.getElementById("output-volume") as HTMLInputElement,
    inputVolumeValue: document.getElementById(
      "input-volume-value",
    ) as HTMLElement,
    outputVolumeValue: document.getElementById(
      "output-volume-value",
    ) as HTMLElement,
    startCapture: document.getElementById("start-capture") as HTMLButtonElement,
    stopCapture: document.getElementById("stop-capture") as HTMLButtonElement,
    exportCsv: document.getElementById("export-csv") as HTMLButtonElement,
    refreshDevices: document.getElementById(
      "refresh-devices",
    ) as HTMLButtonElement,
    statusMessage: document.getElementById("status-message") as HTMLElement,
    captureProgress: document.getElementById("capture-progress") as HTMLElement,
    progressFill: document.getElementById("progress-fill") as HTMLElement,
    resultsContainer: document.getElementById(
      "results-container",
    ) as HTMLElement,
    resultsInfo: document.getElementById("results-info") as HTMLElement,
    captureGraph: document.getElementById("capture-graph") as HTMLCanvasElement,
    graphPlaceholder: document.getElementById(
      "capture-graph-placeholder",
    ) as HTMLElement,
    inputChannelsInfo: document.getElementById(
      "input-channels-info",
    ) as HTMLElement,
    inputSampleRate: document.getElementById(
      "input-sample-rate",
    ) as HTMLElement,
    inputBitDepth: document.getElementById("input-bit-depth") as HTMLElement,
    outputChannelsInfo: document.getElementById(
      "output-channels-info",
    ) as HTMLElement,
    outputSampleRate: document.getElementById(
      "output-sample-rate",
    ) as HTMLElement,
    outputBitDepth: document.getElementById("output-bit-depth") as HTMLElement,
    inputRoutingBtn: document.getElementById(
      "input-routing-btn",
    ) as HTMLButtonElement,
    outputRoutingBtn: document.getElementById(
      "output-routing-btn",
    ) as HTMLButtonElement,
    recallSweepsBtn: document.getElementById(
      "recall-sweeps",
    ) as HTMLButtonElement,
    recallModal: document.getElementById("recall-modal") as HTMLElement,
    recallModalClose: document.getElementById(
      "recall-modal-close",
    ) as HTMLButtonElement,
    recallModalCancel: document.getElementById(
      "recall-modal-cancel",
    ) as HTMLButtonElement,
    sweepCount: document.getElementById("sweep-count") as HTMLElement,
    sweepsList: document.getElementById("sweeps-list") as HTMLElement,
    clearAllSweeps: document.getElementById(
      "clear-all-sweeps",
    ) as HTMLButtonElement,
    phaseToggle: document.getElementById(
      "capture-phase-toggle",
    ) as HTMLInputElement,
    smoothingSelect: document.getElementById(
      "capture-smoothing-select",
    ) as HTMLSelectElement,
  };

  // Store device information
  interface DeviceInfo {
    value: string;
    label: string;
    channels?: number;
    sampleRate?: number;
  }
  const deviceInfo: { input: DeviceInfo[]; output: DeviceInfo[] } = {
    input: [],
    output: [],
  };

  // Routing matrices for input and output
  let inputRoutingMatrix: RoutingMatrix | null = null;
  let outputRoutingMatrix: RoutingMatrix | null = null;

  // Volume slider handlers with visual feedback
  function updateVolumeSlider(
    slider: HTMLInputElement,
    valueDisplay: HTMLElement,
  ) {
    const value = parseInt(slider.value);
    valueDisplay.textContent = `${value}%`;

    // Update slider background gradient
    const percentage = value;
    slider.style.background = `linear-gradient(to right, var(--button-primary) 0%, var(--button-primary) ${percentage}%, var(--bg-accent) ${percentage}%, var(--bg-accent) 100%)`;
  }

  elements.inputVolume.addEventListener("input", (e) => {
    updateVolumeSlider(e.target as HTMLInputElement, elements.inputVolumeValue);
  });

  elements.outputVolume.addEventListener("input", (e) => {
    updateVolumeSlider(
      e.target as HTMLInputElement,
      elements.outputVolumeValue,
    );
  });

  // Initialize slider gradients
  updateVolumeSlider(elements.inputVolume, elements.inputVolumeValue);
  updateVolumeSlider(elements.outputVolume, elements.outputVolumeValue);

  // Update device info badges
  function updateDeviceInfo(deviceName: string, isInput: boolean) {
    const devices = isInput ? deviceInfo.input : deviceInfo.output;
    const device = devices.find((d) => d.value === deviceName);

    if (device && device.channels !== undefined) {
      const channelsBadge = isInput
        ? elements.inputChannelsInfo
        : elements.outputChannelsInfo;
      const sampleRateBadge = isInput
        ? elements.inputSampleRate
        : elements.outputSampleRate;

      // Update channel count
      channelsBadge.textContent = `${device.channels} ch`;
      channelsBadge.classList.add("detected");

      // Update sample rate if available (default to 48kHz)
      const sampleRate = device.sampleRate || 48000;
      sampleRateBadge.textContent =
        sampleRate >= 1000 ? `${sampleRate / 1000}kHz` : `${sampleRate}Hz`;

      // Update or create routing matrix for this device
      if (isInput) {
        if (inputRoutingMatrix) {
          inputRoutingMatrix.updateChannelCount(device.channels);
        } else {
          inputRoutingMatrix = new RoutingMatrix(device.channels);
          inputRoutingMatrix.setOnRoutingChange((routing) => {
            console.log("Input routing changed:", routing);
            // Store routing configuration for use during capture
          });
        }
      } else {
        if (outputRoutingMatrix) {
          outputRoutingMatrix.updateChannelCount(device.channels);
        } else {
          outputRoutingMatrix = new RoutingMatrix(device.channels);
          outputRoutingMatrix.setOnRoutingChange((routing) => {
            console.log("Output routing changed:", routing);
            // Store routing configuration for use during capture
          });
        }
      }
    }
  }

  // Device change handlers
  elements.inputDevice.addEventListener("change", () => {
    updateDeviceInfo(elements.inputDevice.value, true);
  });

  elements.outputDevice.addEventListener("change", () => {
    updateDeviceInfo(elements.outputDevice.value, false);
  });

  // Routing button handlers
  elements.inputRoutingBtn.addEventListener("click", () => {
    if (inputRoutingMatrix) {
      inputRoutingMatrix.show(elements.inputRoutingBtn);
    } else {
      showStatus("Please select an input device first", "info");
    }
  });

  elements.outputRoutingBtn.addEventListener("click", () => {
    if (outputRoutingMatrix) {
      outputRoutingMatrix.show(elements.outputRoutingBtn);
    } else {
      showStatus("Please select an output device first", "info");
    }
  });

  // Show status message
  function showStatus(
    message: string,
    type: "info" | "success" | "error" = "info",
  ) {
    elements.statusMessage.textContent = message;
    elements.statusMessage.className = `capture-status ${type}`;
    setTimeout(() => {
      elements.statusMessage.textContent = "";
    }, 5000);
  }

  // Load audio devices
  async function loadDevices() {
    try {
      showStatus("Loading audio devices...", "info");
      const devices = await captureController.getAudioDevices();

      // Parse device info and store it
      deviceInfo.input = devices.input.map((device) => {
        const parsed = parseDeviceInfo(device.info || "");
        return {
          value: device.value,
          label: device.label,
          channels: parsed.channels,
          sampleRate: parsed.sampleRate,
        };
      });

      deviceInfo.output = devices.output.map((device) => {
        const parsed = parseDeviceInfo(device.info || "");
        return {
          value: device.value,
          label: device.label,
          channels: parsed.channels,
          sampleRate: parsed.sampleRate,
        };
      });

      // Clear and populate input devices
      elements.inputDevice.innerHTML = "";
      devices.input.forEach((device) => {
        const option = document.createElement("option");
        option.value = device.value;
        option.textContent = device.label;
        if (device.info) {
          option.title = device.info;
        }
        elements.inputDevice.appendChild(option);
      });

      // Clear and populate output devices
      elements.outputDevice.innerHTML = "";
      devices.output.forEach((device) => {
        const option = document.createElement("option");
        option.value = device.value;
        option.textContent = device.label;
        if (device.info) {
          option.title = device.info;
        }
        elements.outputDevice.appendChild(option);
      });

      // Update badges for initially selected devices
      if (elements.inputDevice.value) {
        updateDeviceInfo(elements.inputDevice.value, true);
      }
      if (elements.outputDevice.value) {
        updateDeviceInfo(elements.outputDevice.value, false);
      }

      showStatus("Audio devices loaded successfully", "success");
    } catch (error) {
      console.error("Failed to load devices:", error);
      showStatus(
        `Failed to load devices: ${(error as Error).message}`,
        "error",
      );
    }
  }

  // Parse device info string (e.g., "2ch 48kHz" or "8 ch")
  function parseDeviceInfo(info: string): {
    channels?: number;
    sampleRate?: number;
  } {
    const result: { channels?: number; sampleRate?: number } = {};

    // Parse channel count (e.g., "2ch", "8 ch", "2 ch")
    const channelMatch = info.match(/(\d+)\s*ch/i);
    if (channelMatch) {
      result.channels = parseInt(channelMatch[1]);
    }

    // Parse sample rate (e.g., "48kHz", "48000Hz", "96 kHz")
    const sampleRateMatch = info.match(/(\d+(?:\.\d+)?)\s*k?Hz/i);
    if (sampleRateMatch) {
      const value = parseFloat(sampleRateMatch[1]);
      result.sampleRate = info.toLowerCase().includes("khz")
        ? value * 1000
        : value;
    }

    return result;
  }

  // Start capture
  async function startCapture() {
    try {
      // Disable start button, show stop button
      elements.startCapture.disabled = true;
      elements.stopCapture.classList.remove("hidden");
      elements.stopCapture.disabled = false;
      elements.exportCsv.classList.add("hidden");

      // Show progress
      elements.captureProgress.classList.remove("hidden");
      elements.progressFill.style.width = "0%";
      elements.progressFill.textContent = "0%";

      // Hide previous results
      elements.resultsContainer.classList.add("hidden");

      showStatus("Starting capture...", "info");

      const params = {
        inputDevice: elements.inputDevice.value,
        outputDevice: elements.outputDevice.value,
        outputChannel: elements.outputChannel.value as
          | "left"
          | "right"
          | "both"
          | "default",
        signalType: elements.signalType.value as "sweep" | "white" | "pink",
        duration: parseInt(elements.duration.value),
        sampleRate: parseInt(elements.sampleRate.value),
        inputVolume: parseInt(elements.inputVolume.value),
        outputVolume: parseInt(elements.outputVolume.value),
      };

      console.log("Starting capture with params:", params);

      // Simulate progress
      const duration = params.duration * 1000;
      const startTime = Date.now();
      const progressInterval = setInterval(() => {
        const elapsed = Date.now() - startTime;
        const progress = Math.min(100, (elapsed / duration) * 100);
        elements.progressFill.style.width = `${progress}%`;
        elements.progressFill.textContent = `${Math.round(progress)}%`;

        if (progress >= 100) {
          clearInterval(progressInterval);
        }
      }, 100);

      const result = await captureController.startCapture(params);

      clearInterval(progressInterval);
      elements.progressFill.style.width = "100%";
      elements.progressFill.textContent = "100%";

      if (result.success) {
        showStatus("Capture completed successfully!", "success");

        // Store the capture data
        currentCaptureData = {
          frequencies: result.frequencies,
          magnitudes: result.magnitudes,
          phases: result.phases,
          metadata: {
            timestamp: new Date(),
            deviceName:
              elements.inputDevice.options[elements.inputDevice.selectedIndex]
                .text,
            signalType: params.signalType,
            duration: params.duration,
            sampleRate: params.sampleRate,
            outputChannel: params.outputChannel,
          },
        };

        // Debug: Log captured data statistics
        console.log("=== CAPTURE DATA ANALYSIS ===");
        console.log("Frequencies:", result.frequencies.length, "points");
        console.log("First 5 frequencies:", result.frequencies.slice(0, 5));
        console.log("Magnitudes:", result.magnitudes.length, "points");
        console.log("First 5 magnitudes:", result.magnitudes.slice(0, 5));
        console.log(
          "Magnitude range:",
          Math.min(...result.magnitudes),
          "to",
          Math.max(...result.magnitudes),
          "dB",
        );

        // Check if signal level is too low
        const maxMagnitude = Math.max(...currentCaptureData.magnitudes);
        const minMagnitude = Math.min(...currentCaptureData.magnitudes);
        const avgMagnitude =
          currentCaptureData.magnitudes.reduce(
            (a: number, b: number) => a + b,
            0,
          ) / currentCaptureData.magnitudes.length;
        console.log("Signal statistics:");
        console.log("  Max:", maxMagnitude.toFixed(1), "dB");
        console.log("  Min:", minMagnitude.toFixed(1), "dB");
        console.log("  Avg:", avgMagnitude.toFixed(1), "dB");

        if (maxMagnitude < -60) {
          const continueAnyway = confirm(
            `⚠️ Low Signal Level Warning\n\n` +
              `Maximum SPL captured: ${maxMagnitude.toFixed(1)} dB\n\n` +
              `This is very low and may indicate:\n` +
              `• Output volume is muted or too low\n` +
              `• Input gain is too low\n` +
              `• Microphone is not positioned correctly\n\n` +
              `Please increase the volume and try again.\n\n` +
              `Do you want to save this sweep anyway?`,
          );

          if (!continueAnyway) {
            showStatus("Capture discarded due to low signal level", "info");
            return;
          }
        }

        // Compute smoothed magnitudes for storage
        const octaveFraction = parseInt(elements.smoothingSelect.value) || 3;
        const smoothedMags = CaptureGraphRenderer.applySmoothing(
          currentCaptureData.frequencies,
          currentCaptureData.magnitudes,
          octaveFraction,
        );

        let smoothedPhases: number[] = [];
        if (currentCaptureData.phases && currentCaptureData.phases.length > 0) {
          smoothedPhases = CaptureGraphRenderer.applyPhaseSmoothing(
            currentCaptureData.frequencies,
            currentCaptureData.phases,
            octaveFraction,
          );
        }

        // Save to storage
        try {
          CaptureStorage.saveCapture({
            timestamp: currentCaptureData.metadata.timestamp,
            deviceName: currentCaptureData.metadata.deviceName,
            signalType: currentCaptureData.metadata.signalType as
              | "sweep"
              | "white"
              | "pink",
            duration: currentCaptureData.metadata.duration,
            sampleRate: currentCaptureData.metadata.sampleRate,
            outputChannel: currentCaptureData.metadata.outputChannel,
            frequencies: currentCaptureData.frequencies,
            rawMagnitudes: currentCaptureData.magnitudes,
            smoothedMagnitudes: smoothedMags,
            rawPhase: currentCaptureData.phases || [],
            smoothedPhase: smoothedPhases,
          });
          console.log("Capture saved to localStorage");
        } catch (error) {
          console.error("Failed to save capture to storage:", error);
        }

        // Display results
        displayResults(currentCaptureData);

        // Show and enable export button
        elements.exportCsv.classList.remove("hidden");
        elements.exportCsv.disabled = false;
      } else {
        throw new Error(result.error || "Capture failed");
      }
    } catch (error) {
      console.error("Capture error:", error);
      showStatus(`Capture failed: ${(error as Error).message}`, "error");
    } finally {
      // Reset buttons
      elements.startCapture.disabled = false;
      elements.stopCapture.classList.add("hidden");
      elements.stopCapture.disabled = true;

      // Hide progress after a moment
      setTimeout(() => {
        elements.captureProgress.classList.add("hidden");
      }, 2000);
    }
  }

  // Stop capture
  function stopCapture() {
    captureController.stopCapture();
    showStatus("Capture stopped", "info");

    // Reset buttons
    elements.startCapture.disabled = false;
    elements.stopCapture.classList.add("hidden");
    elements.stopCapture.disabled = true;
    elements.captureProgress.classList.add("hidden");
  }

  // Display results
  function displayResults(data: {
    frequencies: number[];
    magnitudes: number[];
    phases?: number[];
    metadata: {
      timestamp: Date;
      deviceName: string;
      signalType: string;
      duration: number;
      sampleRate: number;
      outputChannel: string;
    };
  }) {
    if (!data || !data.frequencies || !data.magnitudes) {
      console.error("displayResults: Invalid data", data);
      return;
    }

    console.log("displayResults called with:", {
      frequencies: data.frequencies.length,
      magnitudes: data.magnitudes.length,
      phases: data.phases?.length || 0,
    });

    // Hide placeholder, show results container
    elements.graphPlaceholder.classList.add("hidden");
    elements.resultsContainer.classList.remove("hidden");

    // Display info
    elements.resultsInfo.innerHTML = `
      <p><strong>Captured:</strong> ${data.frequencies.length} frequency points</p>
      <p><strong>Sample Rate:</strong> ${data.metadata.sampleRate} Hz</p>
      <p><strong>Signal Type:</strong> ${data.metadata.signalType}</p>
      <p><strong>Duration:</strong> ${data.metadata.duration} seconds</p>
    `;

    // Initialize graph renderer if needed
    if (!graphRenderer) {
      console.log("Initializing graph renderer...");
      graphRenderer = new CaptureGraphRenderer(elements.captureGraph);
    }

    // Apply smoothing based on current setting
    const octaveFraction = parseInt(elements.smoothingSelect.value);
    const smoothedMagnitudes = CaptureGraphRenderer.applySmoothing(
      data.frequencies,
      data.magnitudes,
      octaveFraction,
    );

    let smoothedPhase: number[] = [];
    if (data.phases && data.phases.length > 0) {
      smoothedPhase = CaptureGraphRenderer.applyPhaseSmoothing(
        data.frequencies,
        data.phases,
        octaveFraction,
      );
    }

    // Prepare graph data
    const graphData = {
      frequencies: data.frequencies,
      rawMagnitudes: data.magnitudes,
      smoothedMagnitudes: smoothedMagnitudes,
      rawPhase: data.phases || [],
      smoothedPhase: smoothedPhase,
      outputChannel: data.metadata.outputChannel as
        | "left"
        | "right"
        | "both"
        | "default",
    };

    console.log("Rendering graph with data:", graphData);

    // Render the graph
    graphRenderer.renderGraph(graphData);

    console.log("Graph rendering complete");
  }

  // Export CSV
  function exportCsv() {
    if (!currentCaptureData) {
      showStatus("No capture data to export", "error");
      return;
    }

    try {
      const exportData = {
        frequencies: currentCaptureData.frequencies,
        rawMagnitudes: currentCaptureData.magnitudes,
        smoothedMagnitudes: currentCaptureData.magnitudes, // Use same data for now
        rawPhase: currentCaptureData.phases || [],
        smoothedPhase: currentCaptureData.phases || [],
        metadata: {
          ...currentCaptureData.metadata,
          signalType: currentCaptureData.metadata.signalType as
            | "sweep"
            | "white"
            | "pink",
        },
      };

      CSVExporter.exportToCSV(exportData);
      showStatus("CSV exported successfully", "success");
    } catch (error) {
      console.error("Export error:", error);
      showStatus(`Export failed: ${(error as Error).message}`, "error");
    }
  }

  // Recall modal functions
  async function openRecallModal() {
    console.log("Opening recall modal...");
    elements.recallModal.classList.remove("hidden");
    await loadSweepsList();
  }

  function closeRecallModal() {
    elements.recallModal.classList.add("hidden");
  }

  async function loadSweepsList() {
    try {
      const captures = CaptureStorage.getAllCaptures();
      const count = captures.length;

      // Update count
      elements.sweepCount.textContent = `${count} record${count !== 1 ? "s" : ""} saved`;

      // Clear list
      elements.sweepsList.innerHTML = "";

      if (count === 0) {
        elements.sweepsList.innerHTML = `
          <div class="sweeps-empty">
            <p>No saved captures yet. Capture audio to save it automatically.</p>
          </div>
        `;
        return;
      }

      // Render capture items
      captures.forEach((capture) => {
        const item = createSweepItem(capture);
        elements.sweepsList.appendChild(item);
      });
    } catch (error) {
      console.error("Failed to load captures:", error);
      showStatus("Failed to load saved captures", "error");
    }
  }

  function createSweepItem(capture: StoredCapture): HTMLElement {
    const item = document.createElement("div");
    item.className = "sweep-item";

    const timestamp = new Date(capture.timestamp);
    const dateStr = timestamp.toLocaleDateString();
    const timeStr = timestamp.toLocaleTimeString();

    item.innerHTML = `
      <div class="sweep-info">
        <div class="sweep-title">${capture.name}</div>
        <div class="sweep-details">
          <span class="sweep-detail">📅 ${dateStr} ${timeStr}</span>
          <span class="sweep-detail">🎵 ${capture.signalType}</span>
          <span class="sweep-detail">⏱️ ${capture.duration}s</span>
          <span class="sweep-detail">🔊 ${capture.outputChannel}</span>
          <span class="sweep-detail">📊 ${capture.frequencies.length} points</span>
        </div>
      </div>
      <div class="sweep-actions">
        <button class="btn btn-primary btn-sm load-sweep-btn" data-id="${capture.id}">Load</button>
        <button class="btn btn-secondary btn-sm export-sweep-btn" data-id="${capture.id}">Export</button>
        <button class="btn btn-danger btn-sm delete-sweep-btn" data-id="${capture.id}">Delete</button>
      </div>
    `;

    // Add event listeners
    const loadBtn = item.querySelector(".load-sweep-btn") as HTMLButtonElement;
    const exportBtn = item.querySelector(
      ".export-sweep-btn",
    ) as HTMLButtonElement;
    const deleteBtn = item.querySelector(
      ".delete-sweep-btn",
    ) as HTMLButtonElement;

    loadBtn.addEventListener("click", () => loadSweep(capture));
    exportBtn.addEventListener("click", () => exportSweep(capture));
    deleteBtn.addEventListener("click", () => deleteSweep(capture.id));

    return item;
  }

  function loadSweep(capture: StoredCapture) {
    // Load capture data into current view
    currentCaptureData = {
      frequencies: capture.frequencies,
      magnitudes: capture.rawMagnitudes,
      phases: capture.rawPhase,
      metadata: {
        timestamp: new Date(capture.timestamp),
        deviceName: capture.deviceName,
        signalType: capture.signalType,
        duration: capture.duration,
        sampleRate: capture.sampleRate,
        outputChannel: capture.outputChannel,
      },
    };

    // Display results
    displayResults(currentCaptureData);

    // Show export button
    elements.exportCsv.classList.remove("hidden");
    elements.exportCsv.disabled = false;

    // Close modal
    closeRecallModal();

    showStatus("Capture loaded successfully", "success");
  }

  function exportSweep(capture: StoredCapture) {
    try {
      const exportData = {
        frequencies: capture.frequencies,
        rawMagnitudes: capture.rawMagnitudes,
        smoothedMagnitudes: capture.smoothedMagnitudes,
        rawPhase: capture.rawPhase,
        smoothedPhase: capture.smoothedPhase,
        metadata: {
          timestamp: new Date(capture.timestamp),
          deviceName: capture.deviceName,
          signalType: capture.signalType,
          duration: capture.duration,
          sampleRate: capture.sampleRate,
          outputChannel: capture.outputChannel,
        },
      };

      CSVExporter.exportToCSV(exportData);
      showStatus("Capture exported successfully", "success");
    } catch (error) {
      console.error("Export error:", error);
      showStatus(`Export failed: ${(error as Error).message}`, "error");
    }
  }

  function deleteSweep(id: string) {
    if (!confirm("Are you sure you want to delete this capture?")) {
      return;
    }

    try {
      CaptureStorage.deleteCapture(id);
      loadSweepsList();
      showStatus("Capture deleted", "success");
    } catch (error) {
      console.error("Delete error:", error);
      showStatus("Failed to delete capture", "error");
    }
  }

  function clearAllSweeps() {
    if (
      !confirm(
        "Are you sure you want to delete ALL saved captures? This cannot be undone.",
      )
    ) {
      return;
    }

    try {
      CaptureStorage.clearAll();
      loadSweepsList();
      showStatus("All captures cleared", "success");
    } catch (error) {
      console.error("Clear error:", error);
      showStatus("Failed to clear captures", "error");
    }
  }

  // Event handlers
  elements.startCapture.addEventListener("click", startCapture);
  elements.stopCapture.addEventListener("click", stopCapture);
  elements.exportCsv.addEventListener("click", exportCsv);
  elements.refreshDevices.addEventListener("click", loadDevices);

  // Debug recall button
  console.log("Recall button element:", elements.recallSweepsBtn);
  if (elements.recallSweepsBtn) {
    elements.recallSweepsBtn.addEventListener("click", () => {
      console.log("Recall button clicked!");
      openRecallModal();
    });
    console.log("Recall button event listener attached");
  } else {
    console.error("Recall button not found in DOM!");
  }

  elements.recallModalClose.addEventListener("click", closeRecallModal);
  elements.recallModalCancel.addEventListener("click", closeRecallModal);
  elements.clearAllSweeps.addEventListener("click", clearAllSweeps);

  // Close modal on background click
  elements.recallModal.addEventListener("click", (e) => {
    if (e.target === elements.recallModal) {
      closeRecallModal();
    }
  });

  // Graph control handlers
  elements.phaseToggle.addEventListener("change", () => {
    if (graphRenderer) {
      const showPhase = elements.phaseToggle.checked;
      graphRenderer.setPhaseVisibility(showPhase);

      // Re-render if we have data
      if (currentCaptureData) {
        displayResults(currentCaptureData);
      } else {
        graphRenderer.renderPlaceholder();
      }
    }
  });

  elements.smoothingSelect.addEventListener("change", () => {
    // Re-render with new smoothing if we have data
    if (currentCaptureData) {
      displayResults(currentCaptureData);
    }
  });

  // Load devices on startup
  await loadDevices();

  console.log("Audio Capture Demo initialized");
});
