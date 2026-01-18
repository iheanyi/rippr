#!/usr/bin/env python3
"""Generate .icns and .ico files from the master PNG."""

import os
import subprocess
from PIL import Image

script_dir = os.path.dirname(os.path.abspath(__file__))
master_png = os.path.join(script_dir, 'rippr_logo_master.png')

# Load master image
img = Image.open(master_png)

# Generate .ico for Windows (multiple sizes embedded)
ico_sizes = [(16, 16), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)]
ico_images = []
for size in ico_sizes:
    resized = img.resize(size, Image.Resampling.LANCZOS)
    ico_images.append(resized)

ico_path = os.path.join(script_dir, 'icon.ico')
ico_images[0].save(ico_path, format='ICO', sizes=ico_sizes)
print(f"Created: {ico_path}")

# Generate .icns for macOS using iconutil
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

# Use iconutil to create .icns
icns_path = os.path.join(script_dir, 'icon.icns')
subprocess.run(['iconutil', '-c', 'icns', iconset_dir, '-o', icns_path], check=True)
print(f"Created: {icns_path}")

# Clean up iconset directory
import shutil
shutil.rmtree(iconset_dir)

print("Icon generation complete!")
