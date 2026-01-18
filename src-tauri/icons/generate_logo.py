#!/usr/bin/env python3
"""
Rippr Logo Generator v2
Clean, bold design that works well as an app icon.
"""

import math
from PIL import Image, ImageDraw

SIZE = 1024
CENTER = SIZE // 2

# Color palette
DEEP_BLACK = (18, 18, 22)
ELECTRIC_CORAL = (255, 95, 75)
BRIGHT_CORAL = (255, 120, 100)

def create_rippr_logo():
    """Generate the Rippr app icon - clean and bold."""

    # Create canvas with solid dark background
    img = Image.new('RGBA', (SIZE, SIZE), (0, 0, 0, 0))  # Transparent base
    draw = ImageDraw.Draw(img)

    # Draw solid dark circle background (for rounded icon look)
    padding = 40
    draw.ellipse([padding, padding, SIZE - padding, SIZE - padding], fill=DEEP_BLACK)

    # Waveform parameters
    wave_y_center = CENTER
    num_bars = 17
    bar_spacing = SIZE * 0.035
    total_width = (num_bars - 1) * bar_spacing
    start_x = CENTER - total_width / 2

    # Bar heights - creates a waveform envelope shape
    # Peak in the middle, tapering at edges
    heights = []
    for i in range(num_bars):
        # Distance from center (0 to 1)
        dist_from_center = abs(i - (num_bars - 1) / 2) / ((num_bars - 1) / 2)

        # Envelope shape - higher in middle
        envelope = 1.0 - (dist_from_center ** 1.5) * 0.7

        # Add some variation
        variation = 0.85 + 0.15 * math.sin(i * 1.2)

        height = SIZE * 0.32 * envelope * variation
        heights.append(height)

    # Draw waveform bars
    bar_width = SIZE * 0.022

    for i in range(num_bars):
        x = start_x + i * bar_spacing
        height = heights[i]

        y1 = wave_y_center - height / 2
        y2 = wave_y_center + height / 2

        # Gradient effect - brighter in center
        dist_from_center = abs(i - (num_bars - 1) / 2) / ((num_bars - 1) / 2)
        r = int(ELECTRIC_CORAL[0] + (BRIGHT_CORAL[0] - ELECTRIC_CORAL[0]) * (1 - dist_from_center))
        g = int(ELECTRIC_CORAL[1] + (BRIGHT_CORAL[1] - ELECTRIC_CORAL[1]) * (1 - dist_from_center))
        b = int(ELECTRIC_CORAL[2] + (BRIGHT_CORAL[2] - ELECTRIC_CORAL[2]) * (1 - dist_from_center))

        # Draw rounded bar
        radius = bar_width / 2
        draw.rounded_rectangle([x - bar_width/2, y1, x + bar_width/2, y2],
                               radius=radius, fill=(r, g, b))

    return img


if __name__ == '__main__':
    import os
    script_dir = os.path.dirname(os.path.abspath(__file__))

    print("Generating Rippr logo v2...")
    logo = create_rippr_logo()

    # Save high-res master
    master_path = os.path.join(script_dir, 'rippr_logo_master.png')
    logo.save(master_path, 'PNG')
    print(f"Created master: {master_path}")
