#!/bin/bash
# Create a simple icon for AutoEQ menu bar app

# Create a temporary directory for icon generation
ICONSET_DIR="/private/tmp/AutoEQ.iconset"
mkdir -p "$ICONSET_DIR"

# Create a simple icon using SF Symbols or ImageMagick
# For now, we'll use the system's sips tool to create a simple colored icon

# Create a base 1024x1024 image with audio waveform theme
# We'll use a simple colored square as a fallback
cat > /tmp/icon.svg << 'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<svg width="1024" height="1024" xmlns="http://www.w3.org/2000/svg">
  <!-- Background with gradient -->
  <defs>
    <linearGradient id="grad1" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" style="stop-color:#4A90E2;stop-opacity:1" />
      <stop offset="100%" style="stop-color:#357ABD;stop-opacity:1" />
    </linearGradient>
    <filter id="shadow">
      <feDropShadow dx="2" dy="2" stdDeviation="3" flood-opacity="0.3"/>
    </filter>
  </defs>

  <!-- Rounded rectangle background -->
  <rect width="1024" height="1024" rx="180" fill="url(#grad1)"/>

  <!-- Audio waveform icon -->
  <g transform="translate(200, 300)" fill="white" filter="url(#shadow)">
    <!-- Waveform bars -->
    <rect x="0" y="150" width="60" height="120" rx="30"/>
    <rect x="100" y="50" width="60" height="320" rx="30"/>
    <rect x="200" y="100" width="60" height="220" rx="30"/>
    <rect x="300" y="0" width="60" height="420" rx="30"/>
    <rect x="400" y="120" width="60" height="180" rx="30"/>
    <rect x="500" y="80" width="60" height="260" rx="30"/>
  </g>

  <!-- EQ text at bottom -->
  <text x="512" y="900" font-family="SF Pro Display, -apple-system, sans-serif"
        font-size="200" font-weight="bold" text-anchor="middle" fill="white"
        opacity="0.95" filter="url(#shadow)">EQ</text>
</svg>
EOF

# Convert SVG to PNG using built-in tools if available
if command -v qlmanage &> /dev/null; then
    # Use Quick Look to convert SVG to PNG
    qlmanage -t -s 1024 -o /tmp /tmp/icon.svg 2>/dev/null
    if [ -f /tmp/icon.svg.png ]; then
        mv /tmp/icon.svg.png /tmp/icon_1024.png
    fi
fi

# If the above didn't work, create a simple colored PNG
if [ ! -f /tmp/icon_1024.png ]; then
    echo "Creating fallback icon..."
    # Use Python with PIL if available
    python3 << 'PYEOF'
try:
    from PIL import Image, ImageDraw, ImageFont
    import os

    # Create a 1024x1024 image with gradient background
    img = Image.new('RGB', (1024, 1024), color='#4A90E2')
    draw = ImageDraw.Draw(img)

    # Draw simple waveform bars
    bars = [
        (100, 400, 150, 624),   # x1, y1, x2, y2
        (200, 300, 250, 724),
        (300, 350, 350, 674),
        (400, 200, 450, 824),
        (500, 380, 550, 644),
        (600, 320, 650, 704),
        (700, 350, 750, 674),
        (800, 250, 850, 774),
    ]

    for bar in bars:
        draw.rounded_rectangle(bar, radius=25, fill='white')

    # Add EQ text
    try:
        font = ImageFont.truetype("/System/Library/Fonts/SFNS.ttf", 180)
    except:
        font = ImageFont.load_default()

    draw.text((512, 880), "EQ", fill='white', anchor="mm", font=font)

    img.save('/tmp/icon_1024.png')
    print("Icon created successfully")
except ImportError:
    print("PIL not available, skipping icon generation")
    exit(1)
PYEOF
fi

# If still no icon, create a very simple one using sips
if [ ! -f /tmp/icon_1024.png ]; then
    echo "Creating minimal fallback icon..."
    # Create a simple blue square
    cat > /tmp/icon_base.png.txt << 'EOF'
iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==
EOF
    base64 -D /tmp/icon_base.png.txt > /tmp/icon_base.png
    sips -z 1024 1024 /tmp/icon_base.png --out /tmp/icon_1024.png 2>/dev/null || {
        echo "Warning: Could not create icon. App will use default icon."
        exit 0
    }
fi

# Generate all required icon sizes
sizes=(16 32 64 128 256 512 1024)

for size in "${sizes[@]}"; do
    if [ -f /tmp/icon_1024.png ]; then
        sips -z $size $size /tmp/icon_1024.png --out "$ICONSET_DIR/icon_${size}x${size}.png" 2>/dev/null
        # Create @2x versions for retina
        if [ $size -le 512 ]; then
            double=$((size * 2))
            sips -z $double $double /tmp/icon_1024.png --out "$ICONSET_DIR/icon_${size}x${size}@2x.png" 2>/dev/null
        fi
    fi
done

# Convert iconset to icns
if [ -d "$ICONSET_DIR" ] && [ "$(ls -A $ICONSET_DIR)" ]; then
    iconutil -c icns "$ICONSET_DIR" -o "/Users/pierre/src/autoeq/target/sotf-configbar.app/Contents/Resources/AppIcon.icns"
    echo "Icon created successfully at /Users/pierre/src/autoeq/target/sotf-configbar.app/Contents/Resources/AppIcon.icns"
else
    echo "Warning: Icon generation failed. App will use default icon."
fi

# Cleanup
rm -rf "$ICONSET_DIR"

exit 0
