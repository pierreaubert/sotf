import type { Options } from "@wdio/types";
import { spawn, type ChildProcess } from "child_process";
import { join } from "path";

// Path to the Tauri binary (will be built before tests)
const APPLICATION_PATH = join(
  process.cwd(),
  "target",
  "release",
  "sotf"
);

// Reference to tauri-driver process
let tauriDriver: ChildProcess;

export const config: Options.Testrunner = {
  specs: ["./sotf-ui-frontend/tests/e2e-ui/**/*.spec.ts"],
  exclude: [],

  maxInstances: 1,
  capabilities: [
    {
      // Use tauri:options to customize Tauri driver behavior
      // @ts-ignore
      "tauri:options": {
        application: APPLICATION_PATH,
      },
      browserName: "chrome",
      browserVersion: "",
      platformName: "",
      acceptInsecureCerts: true,
    },
  ],

  logLevel: "info",
  bail: 0,
  baseUrl: "http://localhost",
  waitforTimeout: 10000,
  connectionRetryTimeout: 120000,
  connectionRetryCount: 3,

  framework: "mocha",
  reporters: ["spec"],

  mochaOpts: {
    ui: "bdd",
    timeout: 60000,
  },

  // Hooks
  onPrepare: async function (config, capabilities) {
    console.log("🔧 Preparing Tauri WebDriver tests...");

    // Build the Tauri app in release mode
    console.log("📦 Building Tauri application in release mode...");
    const buildProcess = spawn("npm", ["run", "tauri", "build"], {
      stdio: "inherit",
      shell: true,
    });

    await new Promise<void>((resolve, reject) => {
      buildProcess.on("close", (code) => {
        if (code === 0) {
          console.log("✅ Tauri build completed successfully");
          resolve();
        } else {
          reject(new Error(`Build failed with exit code ${code}`));
        }
      });
    });

    // Start tauri-driver
    console.log("🚀 Starting tauri-driver...");
    tauriDriver = spawn("tauri-driver", [], {
      stdio: "inherit",
    });

    // Give tauri-driver time to start
    await new Promise((resolve) => setTimeout(resolve, 2000));
  },

  onComplete: function (exitCode, config, capabilities, results) {
    console.log("🧹 Cleaning up tauri-driver...");
    if (tauriDriver) {
      tauriDriver.kill();
    }
  },

  beforeSession: function (config, capabilities, specs, cid) {
    console.log(`Starting test session for: ${specs}`);
  },

  afterTest: async function (
    test,
    context,
    { error, result, duration, passed, retries }
  ) {
    if (error) {
      console.log(`❌ Test failed: ${test.title}`);
      // Take screenshot on failure
      try {
        const screenshot = await browser.takeScreenshot();
        const fs = require("fs");
        const path = require("path");
        const screenshotDir = path.join(
          process.cwd(),
          "test-results",
          "screenshots"
        );
        fs.mkdirSync(screenshotDir, { recursive: true });

        const timestamp = new Date().toISOString().replace(/[:.]/g, "-");
        const filename = `${test.title.replace(/\s+/g, "-")}-${timestamp}.png`;
        fs.writeFileSync(
          path.join(screenshotDir, filename),
          screenshot,
          "base64"
        );
        console.log(`📸 Screenshot saved: ${filename}`);
      } catch (screenshotError) {
        console.error("Failed to capture screenshot:", screenshotError);
      }
    }
  },
};
