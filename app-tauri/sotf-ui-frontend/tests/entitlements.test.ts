import { describe, it, expect } from "vitest";
import { readFileSync, existsSync } from "fs";
import { join } from "path";

/**
 * Test to verify macOS entitlements are properly configured
 * for code signing and distribution
 */
describe("macOS Entitlements Configuration", () => {
  const entitlementsPath = join(
    __dirname,
    "../../src-tauri/Entitlements.plist",
  );

  it("should have Entitlements.plist file", () => {
    expect(existsSync(entitlementsPath)).toBe(true);
  });

  it("should have valid XML structure", () => {
    const content = readFileSync(entitlementsPath, "utf-8");
    expect(content).toContain('<?xml version="1.0" encoding="UTF-8"?>');
    expect(content).toContain("<!DOCTYPE plist");
    expect(content).toContain('<plist version="1.0">');
    expect(content).toContain("</plist>");
  });

  it("should have app sandbox enabled", () => {
    const content = readFileSync(entitlementsPath, "utf-8");
    expect(content).toContain("com.apple.security.app-sandbox");
    expect(content).toContain("<true/>");
  });

  it("should have audio input permission", () => {
    const content = readFileSync(entitlementsPath, "utf-8");
    expect(content).toContain("com.apple.security.device.audio-input");
  });

  it("should have network client permission", () => {
    const content = readFileSync(entitlementsPath, "utf-8");
    expect(content).toContain("com.apple.security.network.client");
  });

  it("should have file read-write permission", () => {
    const content = readFileSync(entitlementsPath, "utf-8");
    expect(content).toContain(
      "com.apple.security.files.user-selected.read-write",
    );
  });

  it("should have all required entitlements for audio app", () => {
    const content = readFileSync(entitlementsPath, "utf-8");

    const requiredEntitlements = [
      "com.apple.security.app-sandbox",
      "com.apple.security.device.audio-input",
      "com.apple.security.network.client",
      "com.apple.security.files.user-selected.read-write",
    ];

    for (const entitlement of requiredEntitlements) {
      expect(content).toContain(entitlement);
    }
  });
});
