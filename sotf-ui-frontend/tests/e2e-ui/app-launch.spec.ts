/**
 * E2E UI Test: Application Launch and Navigation
 *
 * Tests basic app initialization, window management, and step navigation
 */

describe("SOTF Application Launch", () => {
  before(async function () {
    this.timeout(30000);
    // Browser is automatically launched by WebDriverIO config
  });

  after(async function () {
    // Cleanup is handled by WebDriverIO
  });

  it("should launch the application successfully", async () => {
    // Verify window title
    const title = await browser.getTitle();
    expect(title).to.include("Sound of the future");
  });

  it("should display the main app container", async () => {
    const appContainer = await $(".app");
    await expect(appContainer).toBeDisplayed();
  });

  it("should show Step 1: Choose Use Case as initial step", async () => {
    // Check navigation bar
    const step1Label = await $(".step-nav-item.active");
    await expect(step1Label).toBeDisplayed();

    const step1Text = await step1Label.getText();
    expect(step1Text).to.include("Choose Use Case");
  });

  it("should display all 4 use case options", async () => {
    const useCaseCards = await $$(".use-case-card");
    expect(useCaseCards.length).to.equal(4);

    // Verify each use case is present
    const speakerCard = await $('[data-use-case="speaker"]');
    const headphoneCard = await $('[data-use-case="headphone"]');
    const fileCard = await $('[data-use-case="file"]');
    const playMusicCard = await $('[data-use-case="play-music"]');

    await expect(speakerCard).toBeDisplayed();
    await expect(headphoneCard).toBeDisplayed();
    await expect(fileCard).toBeDisplayed();
    await expect(playMusicCard).toBeDisplayed();
  });

  it("should have steps 2-5 disabled initially", async () => {
    const step2 = await $('[data-step-id="2"]');
    const step3 = await $('[data-step-id="3"]');
    const step4 = await $('[data-step-id="4"]');
    const step5 = await $('[data-step-id="5"]');

    // Check if steps have disabled class or attribute
    const step2Classes = await step2.getAttribute("class");
    const step3Classes = await step3.getAttribute("class");
    const step4Classes = await step4.getAttribute("class");
    const step5Classes = await step5.getAttribute("class");

    expect(step2Classes).to.include("disabled");
    expect(step3Classes).to.include("disabled");
    expect(step4Classes).to.include("disabled");
    expect(step5Classes).to.include("disabled");
  });

  it("should navigate to Step 2 when Speaker use case is selected", async () => {
    const speakerCard = await $('[data-use-case="speaker"]');
    await speakerCard.click();

    // Wait for navigation animation
    await browser.pause(600);

    // Verify we're on step 2
    const activeStep = await $(".step-nav-item.active");
    const stepText = await activeStep.getText();
    expect(stepText).to.include("Data Acquisition");

    // Verify step 2 content is visible
    const step2Container = await $('[data-step="2"]');
    await expect(step2Container).toBeDisplayed();

    // Verify speaker inputs are shown
    const speakerInputs = await $("#speaker_inputs");
    await expect(speakerInputs).toBeDisplayed();
  });

  it("should allow navigating back to Step 1 using Previous button", async () => {
    const prevBtn = await $("#step2_prev_btn");
    await prevBtn.click();

    await browser.pause(400);

    // Verify we're back on step 1
    const activeStep = await $(".step-nav-item.active");
    const stepText = await activeStep.getText();
    expect(stepText).to.include("Use Case");

    const step1Container = await $('[data-step="1"]');
    await expect(step1Container).toBeDisplayed();
  });

  it("should remember previous use case selection", async () => {
    // Speaker should still be selected from previous test
    const speakerCard = await $('[data-use-case="speaker"]');
    const cardClasses = await speakerCard.getAttribute("class");

    expect(cardClasses).to.include("selected");
  });

  it("should enable Play Music workflow and skip to Step 4", async () => {
    // Clear previous selection first
    const speakerCard = await $('[data-use-case="speaker"]');
    const isSelected = (await speakerCard.getAttribute("class")).includes(
      "selected"
    );

    // If already selected, we need to re-click it or select another
    // Let's select Play Music
    const playMusicCard = await $('[data-use-case="play-music"]');
    await playMusicCard.click();

    await browser.pause(600);

    // Should jump directly to Step 4
    const activeStep = await $(".step-nav-item.active");
    const stepText = await activeStep.getText();
    expect(stepText).to.include("Results");

    // Step 4 should be visible
    const step4Container = await $('[data-step="4"]');
    await expect(step4Container).toBeDisplayed();
  });
});
