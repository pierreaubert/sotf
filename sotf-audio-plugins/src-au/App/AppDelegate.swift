// AppDelegate.swift
// SOTF Audio Units - Minimal container app

import Cocoa

@main
class AppDelegate: NSObject, NSApplicationDelegate {
    func applicationDidFinishLaunching(_ aNotification: Notification) {
        // This is just a container app for the Audio Unit extension
        // Users won't run this directly - DAWs will load the .appex
        print("SOTF Audio Units container app loaded")
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        return true
    }
}
