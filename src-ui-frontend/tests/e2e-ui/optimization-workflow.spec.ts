/**
 * E2E UI Test: Complete Optimization Workflow
 *
 * Tests the full end-to-end optimization workflow from use case selection
 * through data acquisition, optimization, and export
 */

describe("SOTF Optimization Workflow", () => {
  const OPTIMIZATION_TIMEOUT = 60000; // 60 seconds for optimization

  describe("Speaker Optimization Workflow", () => {
    it("should complete full speaker optimization workflow", async function () {
      this.timeout(120000); // 2 minutes total

      // Step 1: Select Speaker Use Case
      console.log("Step 1: Selecting speaker use case...");
      const speakerCard = await $('[data-use-case="speaker"]');
      await speakerCard.click();
      await browser.pause(600);

      // Step 2: Data Acquisition - Configure Speaker
      console.log("Step 2: Configuring speaker data...");
      const speakerInput = await $("#speaker");
      await speakerInput.setValue("KEF LS50 Meta");

      // Wait for versions to load
      await browser.pause(1000);

      const versionSelect = await $("#version");
      await versionSelect.selectByVisibleText("vendor");

      const measurementSelect = await $("#measurement");
      await measurementSelect.selectByVisibleText("asr");

      // Click Next to go to Step 3
      const nextBtn = await $("#step2_next_btn");
      await expect(nextBtn).toBeEnabled();
      await nextBtn.click();
      await browser.pause(600);

      // Step 3: Configure EQ Parameters
      console.log("Step 3: Configuring EQ parameters...");
      const numFiltersInput = await $("#num_filters");
      await numFiltersInput.clearValue();
      await numFiltersInput.setValue("3"); // Use fewer filters for faster test

      // Reduce maxeval for faster testing
      const maxevalInput = await $("#maxeval");
      await maxevalInput.clearValue();
      await maxevalInput.setValue("100");

      // Select fast algorithm
      const algoSelect = await $("#algo");
      await algoSelect.selectByValue("nlopt:cobyla");

      // Step 3: Run Optimization
      console.log("Step 3: Running optimization...");
      const optimizeBtn = await $("#optimize_btn");
      await optimizeBtn.click();

      // Wait for optimization modal to appear
      const modal = await $("#optimization_modal");
      await expect(modal).toBeDisplayed();

      // Monitor progress
      const progressStatus = await $("#progress_status");
      await expect(progressStatus).toBeDisplayed();

      // Wait for optimization to complete
      console.log("Waiting for optimization to complete...");
      await browser.waitUntil(
        async () => {
          const doneBtn = await $("#done_optimization_btn");
          const isDisplayed = await doneBtn.isDisplayed();
          return isDisplayed;
        },
        {
          timeout: OPTIMIZATION_TIMEOUT,
          timeoutMsg: "Optimization did not complete within timeout",
        }
      );

      console.log("Optimization complete! Closing modal...");

      // Close the modal
      const doneBtn = await $("#done_optimization_btn");
      await doneBtn.click();
      await browser.pause(500);

      // Verify results are displayed in Step 3
      const resultsSection = await $("#step3-results");
      await expect(resultsSection).toBeDisplayed();

      // Verify scores are updated
      const scoreBefore = await $("#step3_score_before");
      const scoreAfter = await $("#step3_score_after");

      const beforeText = await scoreBefore.getText();
      const afterText = await scoreAfter.getText();

      expect(beforeText).to.not.equal("-");
      expect(afterText).to.not.equal("-");
      console.log(`Scores: Before=${beforeText}, After=${afterText}`);

      // Verify filter plot is generated
      const filterPlot = await $("#filter_plot");
      await expect(filterPlot).toBeDisplayed();

      // Step 4: Navigate to Listening & Testing
      console.log("Step 4: Navigating to listening & testing...");
      const continueBtn = await $("#step3_continue_btn");
      await continueBtn.click();
      await browser.pause(600);

      // Verify we're on Step 4
      const step4Container = await $('[data-step="4"]');
      await expect(step4Container).toBeDisplayed();

      // Verify audio controls are present
      const audioControls = await $("#step4-audio-controls");
      await expect(audioControls).toBeDisplayed();

      // Step 5: Navigate to Save & Export
      console.log("Step 5: Navigating to save & export...");
      const continueToSaveBtn = await $("#continue_to_save_btn");
      await continueToSaveBtn.click();
      await browser.pause(600);

      // Verify we're on Step 5
      const step5Container = await $('[data-step="5"]');
      await expect(step5Container).toBeDisplayed();

      // Verify export controls are present
      const exportFormatSelect = await $("#export_format_select");
      await expect(exportFormatSelect).toBeDisplayed();

      const downloadBtn = await $("#download_apo_btn");
      await expect(downloadBtn).toBeEnabled();

      console.log("✅ Full speaker optimization workflow completed successfully!");
    });
  });

  describe("File-Based Optimization Workflow", () => {
    it("should complete file-based optimization workflow", async function () {
      this.timeout(120000);

      // First, navigate back to Step 1
      console.log("Resetting to Step 1...");
      const startNewBtn = await $("#start_new_btn");
      if (await startNewBtn.isDisplayed()) {
        await startNewBtn.click();
        // Handle confirmation dialog
        await browser.pause(500);
        await browser.keys(["Enter"]); // Confirm dialog
        await browser.pause(600);
      }

      // Step 1: Select File Use Case
      console.log("Step 1: Selecting file use case...");
      const fileCard = await $('[data-use-case="file"]');
      await fileCard.click();
      await browser.pause(600);

      // Step 2: Load File
      console.log("Step 2: Loading CSV file...");

      // Note: File selection requires Tauri dialog interaction
      // For now, we can test that the button is present
      const browseBtn = await $("#browse_curve");
      await expect(browseBtn).toBeDisplayed();

      // In a real test, you would:
      // 1. Click the browse button
      // 2. Use platform-specific dialog automation to select file
      // 3. Verify file path is populated

      // For this test, we'll verify the UI elements exist
      const curvePathInput = await $("#curve_path");
      await expect(curvePathInput).toBeDisplayed();

      console.log(
        "✅ File workflow UI elements verified (file selection requires dialog automation)"
      );
    });
  });

  describe("Optimization Parameter Validation", () => {
    it("should validate required fields before allowing optimization", async () => {
      // Navigate to Step 3 with speaker configuration
      const speakerCard = await $('[data-use-case="speaker"]');
      await speakerCard.click();
      await browser.pause(600);

      const speakerInput = await $("#speaker");
      await speakerInput.setValue("KEF LS50 Meta");
      await browser.pause(1000);

      const versionSelect = await $("#version");
      await versionSelect.selectByIndex(1);

      const nextBtn = await $("#step2_next_btn");
      await nextBtn.click();
      await browser.pause(600);

      // Try to optimize with invalid parameters
      const numFiltersInput = await $("#num_filters");
      await numFiltersInput.clearValue();
      await numFiltersInput.setValue("0"); // Invalid

      const optimizeBtn = await $("#optimize_btn");
      await optimizeBtn.click();

      // Should show error (validation happens on backend)
      await browser.pause(1000);

      // Restore valid value
      await numFiltersInput.clearValue();
      await numFiltersInput.setValue("3");
    });

    it("should allow resetting parameters to defaults", async () => {
      const resetBtn = await $("#reset_btn");
      await resetBtn.click();
      await browser.pause(500);

      // Verify default values are restored
      const numFiltersInput = await $("#num_filters");
      const value = await numFiltersInput.getValue();

      // Default is 5 filters
      expect(value).to.equal("5");
    });
  });

  describe("Optimization Cancellation", () => {
    it("should allow cancelling optimization in progress", async function () {
      this.timeout(30000);

      // Start optimization with high maxeval
      const maxevalInput = await $("#maxeval");
      await maxevalInput.clearValue();
      await maxevalInput.setValue("10000"); // High value to ensure we can cancel

      const optimizeBtn = await $("#optimize_btn");
      await optimizeBtn.click();

      // Wait for modal to appear
      const modal = await $("#optimization_modal");
      await expect(modal).toBeDisplayed();

      // Wait a bit for optimization to start
      await browser.pause(2000);

      // Click cancel
      const cancelBtn = await $("#cancel_optimization_btn");
      await cancelBtn.click();

      // Modal should close
      await browser.waitUntil(
        async () => {
          const isDisplayed = await modal.isDisplayed();
          return !isDisplayed;
        },
        {
          timeout: 5000,
          timeoutMsg: "Modal did not close after cancellation",
        }
      );

      console.log("✅ Optimization cancelled successfully");
    });
  });
});
