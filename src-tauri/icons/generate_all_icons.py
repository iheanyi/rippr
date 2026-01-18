#!/usr/bin/env python3
"""Generate all required icon files from the master PNG."""

import os
import subprocess
import shutil
from PIL import Image

script_dir = os.path.dirname(os.path.abspath(__file__))
master_png = os.path.join(script_dir, 'rippr_logo_master.png')

# Load master image
img = Image.open(master_png).convert('RGBA')

print("Generating all icon sizes...")

# Standard PNG icons
png_sizes = {
    '32x32.png': 32,
    '128x128.png': 128,
    '128x128@2x.png': 256,
    'icon.png': 512,
}

for filename, size in png_sizes.items():
    resized = img.resize((size, size), Image.Resampling.LANCZOS)
    path = os.path.join(script_dir, filename)
    resized.save(path, 'PNG')
    print(f"  Created: {filename}")

# Windows Square icons
square_sizes = {
    'Square30x30Logo.png': 30,
    'Square44x44Logo.png': 44,
    'Square71x71Logo.png': 71,
    'Square89x89Logo.png': 89,
    'Square107x107Logo.png': 107,
    'Square142x142Logo.png': 142,
    'Square150x150Logo.png': 150,
    'Square284x284Logo.png': 284,
    'Square310x310Logo.png': 310,
    'StoreLogo.png': 50,
}

for filename, size in square_sizes.items():
    resized = img.resize((size, size), Image.Resampling.LANCZOS)
    path = os.path.join(script_dir, filename)
    resized.save(path, 'PNG')
    print(f"  Created: {filename}")

# Generate .ico for Windows (properly with all sizes)
ico_path = os.path.join(script_dir, 'icon.ico')
ico_sizes = [16, 24, 32, 48, 64, 128, 256]
ico_images = []

for size in ico_sizes:
    resized = img.resize((size, size), Image.Resampling.LANCZOS)
    ico_images.append(resized)

# Save ICO with all sizes embedded
ico_images[0].save(
    ico_path,
    format='ICO',
    sizes=[(s, s) for s in ico_sizes],
    append_images=ico_images[1:]
)
print(f"  Created: icon.ico")

# Generate .icns for macOS using iconutil
print("Generating macOS .icns...")
iconset_dir = os.path.join(script_dir, 'icon.iconset')
os.makedirs(iconset_dir, exist_ok=True)

icns_sizes = [
    ('icon_16x16.png', 16),
    ('icon_16x16@2x.png', 32),
    ('icon_32x32.png', 32),
    ('icon_32x32@2x.png', 64),
    ('icon_128x128.png', 128),
    ('icon_128x128@2x.png', 256),
    ('icon_256x256.png', 256),
    ('icon_256x256@2x.png', 512),
    ('icon_512x512.png', 512),
    ('icon_512x512@2x.png', 1024),
]

for filename, size in icns_sizes:
    resized = img.resize((size, size), Image.Resampling.LANCZOS)
    resized.save(os.path.join(iconset_dir, filename), 'PNG')

icns_path = os.path.join(script_dir, 'icon.icns')
subprocess.run(['iconutil', '-c', 'icns', iconset_dir, '-o', icns_path], check=True)
print(f"  Created: icon.icns")

# Clean up iconset directory
shutil.rmtree(iconset_dir)

print("\nAll icons generated successfully!")
