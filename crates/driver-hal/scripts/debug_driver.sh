#!/bin/bash
#
# Debug Audio HAL Driver Loading Issues
#

set -e

echo "🔍 Debugging Audio HAL Driver..."
echo ""

# Check both possible installation locations
USER_DRIVER_PATH="$HOME/Library/Audio/Plug-Ins/HAL/sotf.driver"
SYSTEM_DRIVER_PATH="/Library/Audio/Plug-Ins/HAL/sotf.driver"

if [ -d "$USER_DRIVER_PATH" ]; then
    DRIVER_PATH="$USER_DRIVER_PATH"
    echo "✅ Found driver in user location: $DRIVER_PATH"
elif [ -d "$SYSTEM_DRIVER_PATH" ]; then
    DRIVER_PATH="$SYSTEM_DRIVER_PATH"
    echo "✅ Found driver in system location: $DRIVER_PATH"
else
    echo "❌ Driver not installed at either location:"
    echo "   User: $USER_DRIVER_PATH"
    echo "   System: $SYSTEM_DRIVER_PATH"
    exit 1
fi

echo "✅ Driver installed at: $DRIVER_PATH"
echo ""

# Check bundle structure
echo "📦 Bundle structure:"
ls -la "$DRIVER_PATH/Contents/" 2>&1 | head -10
echo ""

# Check binary
BINARY_PATH="$DRIVER_PATH/Contents/MacOS/sotf_driver"
if [ -f "$BINARY_PATH" ]; then
    echo "✅ Binary exists"
    echo "📊 Binary info:"
    file "$BINARY_PATH"
    echo ""

    echo "🔗 Linked frameworks:"
    otool -L "$BINARY_PATH" | head -8
    echo ""

    echo "🔐 Code signing status:"
    codesign -dv "$BINARY_PATH" 2>&1 || echo "⚠️  Not code signed"
    echo ""
else
    echo "❌ Binary not found at: $BINARY_PATH"
    exit 1
fi

# Check Info.plist
echo "📄 Info.plist validation:"
plutil -lint "$DRIVER_PATH/Contents/Info.plist"
echo ""

echo "📝 Bundle Info:"
plutil -p "$DRIVER_PATH/Contents/Info.plist" | grep -A2 "CFBundleName\|CFBundleIdentifier\|CFBundleExecutable\|AudioHALPlugIn"
echo ""

# Check permissions
echo "🔐 Permissions:"
ls -la "$DRIVER_PATH/Contents/MacOS/"
echo ""

# Check for other HAL drivers
echo "📚 All installed HAL drivers:"
echo "System drivers:"
ls -la /Library/Audio/Plug-Ins/HAL/ 2>/dev/null || echo "  None"
echo "User drivers:"
ls -la "$HOME/Library/Audio/Plug-Ins/HAL/" 2>/dev/null || echo "  None"
echo ""

# Check Core Audio daemon status
echo "🔄 Core Audio daemon status:"
launchctl list | grep coreaudio || echo "Not running or not found"
echo ""

# Try to find driver in system logs
echo "📋 Recent Core Audio logs (last 2 minutes):"
log show --last 2m --predicate 'subsystem == "com.apple.audio"' 2>&1 | grep -i "plug\|driver\|bundle\|audiohal" | tail -20 || echo "No relevant logs found"
echo ""

echo "💡 Troubleshooting tips:"
echo ""
echo "1. Code Signing Issue:"
echo "   Modern macOS requires drivers to be code signed."
echo "   Solution: Sign the driver with ad-hoc signature:"
echo "   sudo codesign --force --deep --sign - $DRIVER_PATH"
echo ""
echo "2. Restart Core Audio:"
echo "   sudo launchctl kickstart -k system/com.apple.audio.coreaudiod"
echo ""
echo "3. Check System Integrity Protection (SIP):"
echo "   csrutil status"
echo "   If SIP is enabled, it may block unsigned drivers."
echo ""
echo "4. Check Console.app for errors:"
echo "   Open Console.app and filter for 'coreaudio' or 'AudioHALDriver'"
echo ""
echo "5. Verify the driver loads:"
echo "   system_profiler SPAudioDataType | grep -i 'sotf\|SotF'"
echo ""
