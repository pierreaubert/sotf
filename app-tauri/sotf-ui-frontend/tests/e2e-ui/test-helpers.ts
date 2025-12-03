/**
 * Test Helper Functions for E2E UI Tests
 *
 * Common utilities and reusable functions for WebDriver tests
 */

/**
 * Navigate to a specific step in the workflow
 */
export async function navigateToStep(stepNumber: number): Promise<void> {
  const stepNav = await $(`[data-step-id="${stepNumber}"]`);

  if (await stepNav.isExisting()) {
    await stepNav.click();
    await browser.pause(600); // Wait for animation
  } else {
    throw new Error(`Step ${stepNumber} navigation not found or not enabled`);
  }
}

/**
 * Select a use case from Step 1
 */
export async function selectUseCase(
  useCase: "speaker" | "headphone" | "file" | "play-music" | "capture"
): Promise<void> {
  const useCaseCard = await $(`[data-use-case="${useCase}"]`);
  await useCaseCard.waitForDisplayed({ timeout: 5000 });
  await useCaseCard.click();
  await browser.pause(600); // Wait for navigation
}

/**
 * Fill speaker configuration form
 */
export async function configureSpeaker(
  speaker: string,
  version: string,
  measurement: string = "asr"
): Promise<void> {
  const speakerInput = await $("#speaker");
  await speakerInput.setValue(speaker);
  await browser.pause(1000); // Wait for version list to load

  const versionSelect = await $("#version");
  await versionSelect.selectByValue(version);

  const measurementSelect = await $("#measurement");
  await measurementSelect.selectByValue(measurement);
}

/**
 * Set optimization parameters
 */
export async function setOptimizationParams(params: {
  numFilters?: number;
  maxeval?: number;
  algo?: string;
  minFreq?: number;
  maxFreq?: number;
}): Promise<void> {
  if (params.numFilters !== undefined) {
    const numFiltersInput = await $("#num_filters");
    await numFiltersInput.clearValue();
    await numFiltersInput.setValue(params.numFilters.toString());
  }

  if (params.maxeval !== undefined) {
    const maxevalInput = await $("#maxeval");
    await maxevalInput.clearValue();
    await maxevalInput.setValue(params.maxeval.toString());
  }

  if (params.algo) {
    const algoSelect = await $("#algo");
    await algoSelect.selectByValue(params.algo);
  }

  if (params.minFreq !== undefined) {
    const minFreqInput = await $("#min_freq");
    await minFreqInput.clearValue();
    await minFreqInput.setValue(params.minFreq.toString());
  }

  if (params.maxFreq !== undefined) {
    const maxFreqInput = await $("#max_freq");
    await maxFreqInput.clearValue();
    await maxFreqInput.setValue(params.maxFreq.toString());
  }
}

/**
 * Run optimization and wait for completion
 */
export async function runOptimization(
  timeout: number = 60000
): Promise<void> {
  const optimizeBtn = await $("#optimize_btn");
  await optimizeBtn.click();

  // Wait for modal
  const modal = await $("#optimization_modal");
  await modal.waitForDisplayed({ timeout: 5000 });

  // Wait for completion
  const doneBtn = await $("#done_optimization_btn");
  await doneBtn.waitForDisplayed({
    timeout,
    timeoutMsg: "Optimization did not complete within timeout",
  });

  // Close modal
  await doneBtn.click();
  await browser.pause(500);
}

/**
 * Get optimization scores from results
 */
export async function getOptimizationScores(): Promise<{
  before: string;
  after: string;
  improvement: string;
}> {
  const scoreBefore = await $("#step3_score_before");
  const scoreAfter = await $("#step3_score_after");
  const improvement = await $("#step3_score_improvement");

  return {
    before: await scoreBefore.getText(),
    after: await scoreAfter.getText(),
    improvement: await improvement.getText(),
  };
}

/**
 * Verify plot is displayed and has content
 */
export async function verifyPlotDisplayed(plotId: string): Promise<boolean> {
  const plot = await $(`#${plotId}`);

  if (!(await plot.isDisplayed())) {
    return false;
  }

  // Check if plot has children (Plotly creates SVG elements)
  const plotHtml = await plot.getHTML();
  return plotHtml.includes("plotly") || plotHtml.includes("svg");
}

/**
 * Reset to Step 1
 */
export async function resetToStep1(): Promise<void> {
  const startNewBtn = await $("#start_new_btn");

  if (await startNewBtn.isDisplayed()) {
    await startNewBtn.click();

    // Handle confirmation dialog
    await browser.pause(300);
    await browser.keys(["Enter"]); // Confirm with Enter
    await browser.pause(600);
  } else {
    // Already on step 1 or navigate manually
    const step1Nav = await $('[data-step-id="1"]');
    if (await step1Nav.isEnabled()) {
      await step1Nav.click();
      await browser.pause(600);
    }
  }
}

/**
 * Wait for element and click (with retry)
 */
export async function waitAndClick(
  selector: string,
  timeout: number = 5000
): Promise<void> {
  const element = await $(selector);
  await element.waitForDisplayed({ timeout });
  await element.waitForClickable({ timeout });
  await element.click();
}

/**
 * Get all visible buttons and find by text
 */
export async function findButtonByText(text: string): Promise<WebdriverIO.Element | null> {
  const buttons = await $$("button");

  for (const button of buttons) {
    if (await button.isDisplayed()) {
      const btnText = await button.getText();
      if (btnText.includes(text)) {
        return button;
      }
    }
  }

  return null;
}

/**
 * Take screenshot with custom name
 */
export async function takeScreenshot(name: string): Promise<void> {
  const screenshot = await browser.takeScreenshot();
  const fs = require("fs");
  const path = require("path");

  const dir = path.join(process.cwd(), "test-results", "screenshots");
  fs.mkdirSync(dir, { recursive: true });

  const timestamp = new Date().toISOString().replace(/[:.]/g, "-");
  const filename = `${name}-${timestamp}.png`;

  fs.writeFileSync(path.join(dir, filename), screenshot, "base64");
  console.log(`📸 Screenshot saved: ${filename}`);
}

/**
 * Get current active step number
 */
export async function getCurrentStep(): Promise<number> {
  const activeNav = await $(".step-nav-item.active");
  const stepId = await activeNav.getAttribute("data-step-id");
  return parseInt(stepId || "1", 10);
}

/**
 * Check if step is enabled
 */
export async function isStepEnabled(stepNumber: number): Promise<boolean> {
  const stepNav = await $(`[data-step-id="${stepNumber}"]`);
  const classes = await stepNav.getAttribute("class");
  return !classes.includes("disabled");
}

/**
 * Wait for optimization modal to close
 */
export async function waitForModalClose(): Promise<void> {
  const modal = await $("#optimization_modal");
  await browser.waitUntil(
    async () => {
      const isDisplayed = await modal.isDisplayed();
      return !isDisplayed;
    },
    {
      timeout: 5000,
      timeoutMsg: "Modal did not close",
    }
  );
}

/**
 * Log test message with timestamp
 */
export function log(message: string): void {
  const timestamp = new Date().toISOString();
  console.log(`[${timestamp}] ${message}`);
}

/**
 * Create test context object for sharing state between tests
 */
export interface TestContext {
  optimizedFilters?: any[];
  lastScores?: {
    before: number;
    after: number;
  };
  loadedSpeaker?: string;
}

export function createTestContext(): TestContext {
  return {};
}
