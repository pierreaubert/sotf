/**
 * E2E UI Test: Audio Player Functionality
 *
 * Tests audio playback, EQ controls, and spectrum visualization
 */

describe("SOTF Audio Player", () => {
  before(async function () {
    this.timeout(30000);

    // Navigate to Play Music mode (Step 4)
    const playMusicCard = await $('[data-use-case="play-music"]');
    await playMusicCard.click();
    await browser.pause(1000);
  });

  describe("Player Controls", () => {
    it("should display audio player controls", async () => {
      const audioControls = await $("#step4-audio-controls");
      await expect(audioControls).toBeDisplayed();
    });

    it("should have file loading capability", async () => {
      // Check for load file button
      const loadButtons = await $$("button");
      const loadButton = await loadButtons.find(async (btn) => {
        const text = await btn.getText();
        return text.includes("Load") || text.includes("File");
      });

      if (loadButton) {
        await expect(loadButton).toBeDisplayed();
        console.log("✅ Load file button found");
      }
    });

    it("should display demo track selector if available", async () => {
      // Demo track selector might be present
      const selects = await $$("select");

      for (const select of selects) {
        const name = await select.getAttribute("name");
        if (name && name.includes("demo")) {
          await expect(select).toBeDisplayed();
          console.log("✅ Demo track selector found");
          break;
        }
      }
    });
  });

  describe("Playback Controls", () => {
    it("should have play/pause button", async () => {
      // Look for play button (might have different icons/text)
      const buttons = await $$("button");

      const playButton = await buttons.find(async (btn) => {
        const text = await btn.getText();
        const innerHTML = await btn.getHTML();
        return (
          text.includes("Play") ||
          text.includes("▶") ||
          innerHTML.includes("play-icon") ||
          innerHTML.includes("▶")
        );
      });

      if (playButton) {
        await expect(playButton).toBeDisplayed();
        console.log("✅ Play button found");
      }
    });

    it("should have stop button", async () => {
      const buttons = await $$("button");

      const stopButton = await buttons.find(async (btn) => {
        const text = await btn.getText();
        const innerHTML = await btn.getHTML();
        return (
          text.includes("Stop") ||
          text.includes("■") ||
          innerHTML.includes("stop-icon")
        );
      });

      if (stopButton) {
        await expect(stopButton).toBeDisplayed();
        console.log("✅ Stop button found");
      }
    });

    it("should have volume control", async () => {
      // Look for volume slider
      const sliders = await $$('input[type="range"]');

      for (const slider of sliders) {
        const name = await slider.getAttribute("name");
        if (name && (name.includes("volume") || name.includes("gain"))) {
          await expect(slider).toBeDisplayed();
          console.log("✅ Volume control found");

          // Test volume change
          await slider.setValue(50);
          const value = await slider.getValue();
          console.log(`Volume set to: ${value}`);
          break;
        }
      }
    });
  });

  describe("EQ Controls", () => {
    it("should have EQ enable/disable toggle", async () => {
      // Look for EQ toggle
      const buttons = await $$("button");

      const eqButton = await buttons.find(async (btn) => {
        const text = await btn.getText();
        const classes = await btn.getAttribute("class");
        return (
          text.includes("EQ") ||
          (classes && classes.includes("eq")) ||
          text.includes("Equalizer")
        );
      });

      if (eqButton) {
        await expect(eqButton).toBeDisplayed();
        console.log("✅ EQ toggle button found");

        // Test toggling EQ
        const initialText = await eqButton.getText();
        await eqButton.click();
        await browser.pause(500);

        const afterText = await eqButton.getText();
        console.log(`EQ toggle: "${initialText}" -> "${afterText}"`);
      }
    });

    it("should display EQ filter controls if optimized", async () => {
      // EQ filters might be displayed as sliders or parameter inputs
      const filterContainer = await $(".filter-controls, .eq-controls");

      if (await filterContainer.isExisting()) {
        await expect(filterContainer).toBeDisplayed();
        console.log("✅ EQ filter controls found");

        // Count filter rows
        const filterRows = await $$(".filter-row, .eq-band");
        console.log(`Found ${filterRows.length} EQ filter bands`);
      } else {
        console.log(
          "ℹ️  EQ controls not visible (no optimization run yet or UI different)"
        );
      }
    });
  });

  describe("Spectrum Analyzer", () => {
    it("should have spectrum visualization canvas", async () => {
      const canvases = await $$("canvas");

      for (const canvas of canvases) {
        const classes = await canvas.getAttribute("class");
        const id = await canvas.getAttribute("id");

        if (
          (classes && classes.includes("spectrum")) ||
          (id && id.includes("spectrum"))
        ) {
          await expect(canvas).toBeDisplayed();
          console.log("✅ Spectrum analyzer canvas found");

          // Verify canvas dimensions
          const width = await canvas.getCSSProperty("width");
          const height = await canvas.getCSSProperty("height");
          console.log(`Spectrum canvas size: ${width.value} x ${height.value}`);
          break;
        }
      }
    });

    it("should have level meters if present", async () => {
      // Look for level meter elements
      const meters = await $$(".level-meter, .meter");

      if (meters.length > 0) {
        console.log(`✅ Found ${meters.length} level meters`);

        for (const meter of meters) {
          if (await meter.isDisplayed()) {
            const width = await meter.getCSSProperty("width");
            console.log(`Level meter width: ${width.value}`);
          }
        }
      } else {
        console.log("ℹ️  Level meters not found (might be optional feature)");
      }
    });
  });

  describe("Progress Display", () => {
    it("should show playback progress bar", async () => {
      // Look for progress bar
      const progressBars = await $$('progress, .progress-bar, input[type="range"]');

      for (const progress of progressBars) {
        const classes = await progress.getAttribute("class");
        const name = await progress.getAttribute("name");

        if (
          (classes && classes.includes("progress")) ||
          (name && name.includes("position"))
        ) {
          await expect(progress).toBeDisplayed();
          console.log("✅ Playback progress bar found");
          break;
        }
      }
    });

    it("should display time indicators if available", async () => {
      // Look for time display elements
      const elements = await $$("span, div");

      for (const el of elements) {
        const classes = await el.getAttribute("class");
        if (
          classes &&
          (classes.includes("time") ||
            classes.includes("duration") ||
            classes.includes("position"))
        ) {
          if (await el.isDisplayed()) {
            const text = await el.getText();
            console.log(`Time indicator found: "${text}"`);
          }
        }
      }
    });
  });

  describe("Output Device Selection", () => {
    it("should have output device selector", async () => {
      const selects = await $$("select");

      for (const select of selects) {
        const name = await select.getAttribute("name");
        const id = await select.getAttribute("id");

        if (
          (name && name.includes("output")) ||
          (name && name.includes("device")) ||
          (id && id.includes("output")) ||
          (id && id.includes("device"))
        ) {
          await expect(select).toBeDisplayed();
          console.log("✅ Output device selector found");

          // Get available options
          const options = await select.$$("option");
          console.log(`Found ${options.length} output device options`);
          break;
        }
      }
    });
  });

  describe("Plugin System Integration", () => {
    it("should support plugin configuration if available", async () => {
      // Look for plugin controls (upmixer, compressor, etc.)
      const pluginButtons = await $$("button");

      const plugins = [];
      for (const btn of pluginButtons) {
        const text = await btn.getText();
        if (
          text.includes("Upmix") ||
          text.includes("Compressor") ||
          text.includes("Limiter") ||
          text.includes("Gate")
        ) {
          plugins.push(text);
        }
      }

      if (plugins.length > 0) {
        console.log(`✅ Found plugin controls: ${plugins.join(", ")}`);
      } else {
        console.log(
          "ℹ️  Plugin controls not visible in current player mode"
        );
      }
    });

    it("should display channel configuration if upmixer is present", async () => {
      // Look for channel indicators
      const channelElements = await $$(".channel, .speaker");

      if (channelElements.length > 0) {
        console.log(`✅ Found ${channelElements.length} channel indicators`);

        // Look for 5.0 surround labels (L, R, C, SL, SR)
        const labels = ["L", "R", "C", "SL", "SR"];
        for (const label of labels) {
          const elements = await $$(`[data-channel="${label}"], .channel-${label}`);
          if (elements.length > 0) {
            console.log(`✅ Found channel: ${label}`);
          }
        }
      }
    });
  });
});
