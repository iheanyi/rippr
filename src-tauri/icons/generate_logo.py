#!/usr/bin/env python3
"""
Rippr Logo Generator
Based on "Sonic Fracture" design philosophy

Creates an app icon that embodies the act of sonic capture -
waveforms fractured and claimed, precision meeting controlled chaos.
"""

import math
from PIL import Image, ImageDraw, ImageFont, ImageFilter
import os

# Dimensions for high-res icon
SIZE = 1024
CENTER = SIZE // 2

# Color palette - late night studio aesthetic
DEEP_BLACK = (12, 12, 14)
ELECTRIC_CORAL = (255, 95, 75)
WARM_GLOW = (255, 140, 90)
VOID_GRAY = (28, 28, 32)

def create_rippr_logo():
    """Generate the Rippr app icon."""

    # Create canvas with deep black background
    img = Image.new('RGBA', (SIZE, SIZE), DEEP_BLACK)
    draw = ImageDraw.Draw(img)

    # Draw subtle background texture - circular gradient
    for r in range(SIZE // 2, 0, -2):
        alpha = int(15 * (r / (SIZE // 2)))
        color = (VOID_GRAY[0], VOID_GRAY[1], VOID_GRAY[2], alpha)
        draw.ellipse([CENTER - r, CENTER - r, CENTER + r, CENTER + r], fill=color)

    # The icon: A stylized waveform being "ripped" / captured
    # Represents the moment of extraction

    # Main waveform visualization - fractured sine wave segments
    wave_height = SIZE * 0.35
    wave_y_center = CENTER
    num_segments = 24
    segment_width = SIZE * 0.65 / num_segments
    start_x = SIZE * 0.175

    # Create waveform path points with intentional "tear" gaps
    segments = []
    tear_indices = {7, 8, 15, 16}  # Where the fracture occurs

    for i in range(num_segments):
        if i in tear_indices:
            continue  # Skip these to create the "rip" gap

        x = start_x + i * segment_width

        # Vary amplitude for visual interest - like a real audio waveform
        amp_mod = 0.6 + 0.4 * math.sin(i * 0.5)
        if i < 7:
            amp_mod *= 0.7 + (i / 7) * 0.3
        elif i > 16:
            remaining = num_segments - i
            amp_mod *= 0.5 + (remaining / 8) * 0.5

        height = wave_height * amp_mod

        # Each segment is a vertical bar
        bar_width = segment_width * 0.65
        segments.append({
            'x': x,
            'y1': wave_y_center - height / 2,
            'y2': wave_y_center + height / 2,
            'width': bar_width,
            'amp': amp_mod
        })

    # Draw waveform segments with gradient effect
    for seg in segments:
        # Main bar
        x1 = seg['x']
        x2 = seg['x'] + seg['width']

        # Draw with rounded ends for polish
        rect_y1 = seg['y1'] + seg['width'] / 2
        rect_y2 = seg['y2'] - seg['width'] / 2

        # Color based on amplitude - brighter for taller bars
        intensity = seg['amp']
        r = int(ELECTRIC_CORAL[0] * 0.7 + WARM_GLOW[0] * 0.3 * intensity)
        g = int(ELECTRIC_CORAL[1] * 0.7 + WARM_GLOW[1] * 0.3 * intensity)
        b = int(ELECTRIC_CORAL[2] * 0.7 + WARM_GLOW[2] * 0.3 * intensity)
        color = (r, g, b)

        # Draw rounded rectangle
        radius = seg['width'] / 2
        draw.rounded_rectangle([x1, seg['y1'], x2, seg['y2']], radius=radius, fill=color)

    # Add the "rip" effect - diagonal tear marks at the fracture point
    tear_x = start_x + 7.5 * segment_width
    tear_width = segment_width * 2

    # Jagged tear lines
    tear_color = ELECTRIC_CORAL
    tear_points_left = [
        (tear_x - 5, wave_y_center - wave_height * 0.4),
        (tear_x + 8, wave_y_center - wave_height * 0.15),
        (tear_x - 3, wave_y_center + wave_height * 0.1),
        (tear_x + 10, wave_y_center + wave_height * 0.35),
    ]

    tear_points_right = [
        (tear_x + tear_width + 5, wave_y_center - wave_height * 0.38),
        (tear_x + tear_width - 8, wave_y_center - wave_height * 0.12),
        (tear_x + tear_width + 3, wave_y_center + wave_height * 0.13),
        (tear_x + tear_width - 6, wave_y_center + wave_height * 0.37),
    ]

    # Draw tear lines with slight glow
    for i in range(len(tear_points_left) - 1):
        draw.line([tear_points_left[i], tear_points_left[i + 1]],
                  fill=tear_color, width=3)
    for i in range(len(tear_points_right) - 1):
        draw.line([tear_points_right[i], tear_points_right[i + 1]],
                  fill=tear_color, width=3)

    # Add glowing orb effect in the tear gap - the "captured" moment
    glow_center = (tear_x + tear_width / 2, wave_y_center)
    glow_radius = segment_width * 1.2

    # Multi-layer glow
    for r in range(int(glow_radius * 2), 0, -3):
        alpha = int(80 * (1 - r / (glow_radius * 2)))
        glow_color = (ELECTRIC_CORAL[0], ELECTRIC_CORAL[1], ELECTRIC_CORAL[2], alpha)
        draw.ellipse([
            glow_center[0] - r,
            glow_center[1] - r,
            glow_center[0] + r,
            glow_center[1] + r
        ], fill=glow_color)

    # Core bright point
    draw.ellipse([
        glow_center[0] - glow_radius * 0.3,
        glow_center[1] - glow_radius * 0.3,
        glow_center[0] + glow_radius * 0.3,
        glow_center[1] + glow_radius * 0.3
    ], fill=(255, 200, 180))

    # Add subtle outer ring - representing the "capture zone"
    ring_radius = SIZE * 0.42
    ring_width = 3
    draw.ellipse([
        CENTER - ring_radius - ring_width,
        CENTER - ring_radius - ring_width,
        CENTER + ring_radius + ring_width,
        CENTER + ring_radius + ring_width
    ], outline=(ELECTRIC_CORAL[0], ELECTRIC_CORAL[1], ELECTRIC_CORAL[2], 60), width=ring_width)

    # Add small accent marks - like frequency indicators
    for angle in [45, 135, 225, 315]:
        rad = math.radians(angle)
        inner_r = ring_radius - 15
        outer_r = ring_radius + 15
        x1 = CENTER + inner_r * math.cos(rad)
        y1 = CENTER + inner_r * math.sin(rad)
        x2 = CENTER + outer_r * math.cos(rad)
        y2 = CENTER + outer_r * math.sin(rad)
        draw.line([(x1, y1), (x2, y2)], fill=ELECTRIC_CORAL, width=2)

    return img


def create_icon_sizes(base_img, output_dir):
    """Generate all required icon sizes from base image."""

    sizes = {
        'icon.png': 512,
        '32x32.png': 32,
        '128x128.png': 128,
        '128x128@2x.png': 256,
    }

    for filename, size in sizes.items():
        resized = base_img.resize((size, size), Image.Resampling.LANCZOS)
        output_path = os.path.join(output_dir, filename)
        resized.save(output_path, 'PNG')
        print(f"Created: {output_path}")


if __name__ == '__main__':
    script_dir = os.path.dirname(os.path.abspath(__file__))

    print("Generating Rippr logo based on 'Sonic Fracture' philosophy...")
    logo = create_rippr_logo()

    # Save high-res master
    master_path = os.path.join(script_dir, 'rippr_logo_master.png')
    logo.save(master_path, 'PNG')
    print(f"Created master: {master_path}")

    # Generate icon sizes
    create_icon_sizes(logo, script_dir)

    print("\nLogo generation complete!")
