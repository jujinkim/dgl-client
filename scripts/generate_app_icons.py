#!/usr/bin/env python3
"""Regenerate checked-in app icons. Only this opt-in tool requires Pillow."""
import argparse
from io import BytesIO
import json
from pathlib import Path
import struct

from PIL import Image, ImageChops, ImageDraw

ROOT = Path(__file__).resolve().parents[1]
ART = Path("assets/app-icon")
RES = Path("androidApp/src/main/res")
BACKGROUND = "#11151B"
IVORY = "#F3EBDD"
LIME = "#C7F43B"
# Radial background: lighter charcoal behind the symbol, darker toward the edges.
# Center and radius are fractions of the visible tile edge.
BG_CENTER = "#232B38"
BG_EDGE = "#0A0D12"
GRADIENT_CENTER = (0.5, 0.36)
GRADIENT_RADIUS = 0.78
# Tile border for platforms that do not mask or outline icons (Windows, MSIX, dashboard, macOS tile):
# an outer dark hairline separates the tile from light surfaces and an inner light rim,
# brighter at the top, defines the edge on dark surfaces. Small sizes use one mid-tone rim.
EDGE_ALPHA = 0.45
RIM_ALPHA = (0.16, 0.05)
SMALL_RIM = "#3A4352"
SMALL_TILE = 32
ICO_SIZES = (16, 24, 32, 48, 64, 128, 256)
# Rounded-tile corner radius as a fraction of the tile edge, shared with the macOS tile (196/896).
TILE_RADIUS = 196 / 896
MSIX_LOGOS = (("Square44x44Logo", 44), ("Square150x150Logo", 150), ("StoreLogo", 50))
# 64px is the 32pt @2x entry; use the standard macOS point/Retina slots.
ICNS_TYPES = ((b"icp4", 16), (b"icp5", 32),
              (b"ic07", 128), (b"ic08", 256), (b"ic09", 512), (b"ic10", 1024),
              (b"ic11", 32), (b"ic12", 64), (b"ic13", 256), (b"ic14", 512))
# Symbol size relative to the tile edge (desktop, MSIX, dashboard, in-app icon), scaled about the center.
TILE_ART_SCALE = 0.84
# Android: diameter of the circle the symbol is fitted into, inside the 66dp safe zone of the 108dp layer.
ANDROID_ART_DP = 54
# Supersampling factor for antialiased edges.
SS = 4


def png(image):
    data = BytesIO()
    image.save(data, format="PNG", optimize=True)
    return data.getvalue()


def resize(image, size):
    return image.resize((size, size), Image.Resampling.LANCZOS)


def paint_contour(draw, contour, scale, fill, center=0, factor=1):
    """Draw an M/L/C/Z contour, using the same geometry for every platform."""
    points = []
    for command, *values in contour:
        if command in ("M", "L"):
            current = tuple(values)
            points.append(current)
        elif command == "C":
            start = current
            for step in range(1, 33):
                t = step / 32
                points.append(tuple((1-t)**3 * start[axis]
                                    + 3*(1-t)**2*t * values[axis]
                                    + 3*(1-t)*t*t * values[axis+2]
                                    + t**3 * values[axis+4] for axis in (0, 1)))
            current = tuple(values[4:])
        elif command != "Z":
            raise ValueError(f"Unsupported artwork command: {command}")
    draw.polygon([tuple(round(((axis-center)*factor+center)*scale) for axis in point)
                  for point in points], fill=fill)


def radial(size, center, radius):
    """Opaque radial gradient; center and radius are in pixels of a size x size canvas."""
    diameter = max(2, round(radius * 2))
    mask = Image.new("L", (size, size), 255)
    # radial_gradient is 0 at its center and 255 at its inscribed circle.
    mask.paste(Image.radial_gradient("L").resize((diameter, diameter), Image.Resampling.BICUBIC),
               (round(center[0] - diameter / 2), round(center[1] - diameter / 2)))
    return Image.composite(Image.new("RGBA", (size, size), BG_EDGE),
                           Image.new("RGBA", (size, size), BG_CENTER), mask)


def rounded_mask(size, inset=0):
    """Antialiased rounded-tile mask, optionally inset by `inset` pixels."""
    big = size * SS
    mask = Image.new("L", (big, big))
    pad = round(inset * SS)
    ImageDraw.Draw(mask).rounded_rectangle((pad, pad, big - 1 - pad, big - 1 - pad),
                                           radius=max(0, round(big * TILE_RADIUS) - pad), fill=255)
    return resize(mask, size)


def artwork(geometry, size, factor=1):
    """Ivory open terminal frame, transparent panes and lime details; only edges are blended."""
    scale = size * SS / geometry["viewport"]
    center = geometry["viewport"] / 2
    canvas = Image.new("RGBA", (size * SS, size * SS))
    draw = ImageDraw.Draw(canvas)
    for index, contour in enumerate(geometry["contours"]):
        # Pane holes replace pixels with transparency so the background shows through.
        paint_contour(draw, contour, scale, IVORY if index == 0 else (0, 0, 0, 0), center, factor)
    for contour in geometry["details"]:
        paint_contour(draw, contour, scale, LIME, center, factor)
    return resize(canvas, size)


def tile(geometry, size):
    """Render the full-bleed rounded tile directly at `size` so small borders stay crisp."""
    image = radial(size, (size * GRADIENT_CENTER[0], size * GRADIENT_CENTER[1]), size * GRADIENT_RADIUS)
    outer = rounded_mask(size)
    if size <= SMALL_TILE:
        ring = ImageChops.subtract(outer, rounded_mask(size, 1))
        layer = Image.new("RGBA", (size, size), SMALL_RIM)
        layer.putalpha(ring)
        image.alpha_composite(layer)
    else:
        edge = max(1, size / 512)
        rim = max(1, size / 170)
        inner = rounded_mask(size, edge)
        light = ImageChops.subtract(inner, rounded_mask(size, edge + rim))
        top, bottom = (round(255 * alpha) for alpha in RIM_ALPHA)
        fade = Image.linear_gradient("L").resize((size, size)).point(
            lambda value: round(top + (bottom - top) * value / 255))
        layer = Image.new("RGBA", (size, size), "white")
        layer.putalpha(ImageChops.multiply(light, fade))
        image.alpha_composite(layer)
        layer = Image.new("RGBA", (size, size), "black")
        layer.putalpha(ImageChops.subtract(outer, inner).point(lambda value: round(value * EDGE_ALPHA)))
        image.alpha_composite(layer)
    image.putalpha(outer)
    image.alpha_composite(artwork(geometry, size, TILE_ART_SCALE))
    return image


def monochrome(geometry, size, factor):
    scale = size * SS / geometry["viewport"]
    mask = Image.new("L", (size * SS, size * SS))
    draw = ImageDraw.Draw(mask)
    for index, contour in enumerate(geometry["contours"]):
        paint_contour(draw, contour, scale, 255 if index == 0 else 0,
                      geometry["viewport"] / 2, factor)
    for contour in geometry["details"]:
        paint_contour(draw, contour, scale, 255, geometry["viewport"] / 2, factor)
    return resize(mask, size)


def path_data(contours):
    return " ".join(" ".join(map(str, segment)) for contour in contours for segment in contour)


def vector(geometry, factor, size=108):
    viewport = geometry["viewport"]
    return ('<?xml version="1.0" encoding="utf-8"?>\n'
            '<!-- Generated by scripts/generate_app_icons.py; edit assets/app-icon/monochrome.json. -->\n'
            '<vector xmlns:android="http://schemas.android.com/apk/res/android"\n'
            f'    android:width="{size}dp" android:height="{size}dp"\n'
            f'    android:viewportWidth="{viewport}" android:viewportHeight="{viewport}">\n'
            f'    <group android:pivotX="{viewport / 2:g}" android:pivotY="{viewport / 2:g}"\n'
            f'        android:scaleX="{factor:.9f}" android:scaleY="{factor:.9f}">\n'
            f'        <path android:fillColor="#FFFFFF" android:fillType="evenOdd" android:pathData="{path_data(geometry["contours"])}" />\n'
            f'        <path android:fillColor="#FFFFFF" android:pathData="{path_data(geometry["details"])}" />\n'
            '    </group>\n'
            '</vector>\n').encode()


def layer_gradient():
    """Adaptive-icon gradient in 108dp layer units, matched to the visible central 72dp."""
    return (18 + 72 * GRADIENT_CENTER[0], 18 + 72 * GRADIENT_CENTER[1], 72 * GRADIENT_RADIUS)


def background_vector():
    x, y, radius = layer_gradient()
    return ('<?xml version="1.0" encoding="utf-8"?>\n'
            '<!-- Generated by scripts/generate_app_icons.py; edit the palette there. -->\n'
            '<vector xmlns:android="http://schemas.android.com/apk/res/android"\n'
            '    xmlns:aapt="http://schemas.android.com/aapt"\n'
            '    android:width="108dp" android:height="108dp"\n'
            '    android:viewportWidth="108" android:viewportHeight="108">\n'
            '    <path android:pathData="M 0 0 L 108 0 L 108 108 L 0 108 Z">\n'
            '        <aapt:attr name="android:fillColor">\n'
            f'            <gradient android:type="radial" android:centerX="{x:g}" android:centerY="{y:g}"\n'
            f'                android:gradientRadius="{radius:g}"\n'
            f'                android:startColor="#FF{BG_CENTER[1:]}" android:endColor="#FF{BG_EDGE[1:]}" />\n'
            '        </aapt:attr>\n'
            '    </path>\n'
            '</vector>\n').encode()


def icns(image):
    chunks = []
    for kind, size in ICNS_TYPES:
        data = png(resize(image, size))
        chunks.append(kind + struct.pack(">I", len(data) + 8) + data)
    body = b"".join(chunks)
    return b"icns" + struct.pack(">I", len(body) + 8) + body


def compose_vector(geometry):
    """Use the same full-color app icon in the mobile header without raster scaling."""
    viewport = geometry["viewport"]

    def color(value):
        return f"Color(0xff{value[1:]})"

    square = [[["M", 0, 0], ["L", viewport, 0], ["L", viewport, viewport], ["L", 0, viewport], ["Z"]]]
    gradient = (f"Brush.radialGradient(0f to {color(BG_CENTER)}, 1f to {color(BG_EDGE)}, "
                f"center = Offset({viewport * GRADIENT_CENTER[0]:g}f, {viewport * GRADIENT_CENTER[1]:g}f), "
                f"radius = {viewport * GRADIENT_RADIUS:g}f)")
    rows = [(square, "", gradient),
            (geometry["contours"], "pathFillType = PathFillType.EvenOdd, ", f"SolidColor({color(IVORY)})"),
            (geometry["details"], "", f"SolidColor({color(LIME)})")]
    paths = [f'        addPath(PathParser().parsePathString("{path_data(contours)}").toNodes(), '
             f"{fill_type}fill = {fill})" for contours, fill_type, fill in rows]
    pivot = f"{viewport / 2:g}f"
    group = (f"        addGroup(pivotX = {pivot}, pivotY = {pivot}, "
             f"scaleX = {TILE_ART_SCALE:g}f, scaleY = {TILE_ART_SCALE:g}f)")
    body = "\n".join([paths[0], group, *paths[1:], "        clearGroup()"])
    return ("// Generated by scripts/generate_app_icons.py; edit assets/app-icon/monochrome.json.\n"
            "package kim.jujin.dglc.ui\n\n"
            "import androidx.compose.ui.geometry.Offset\n"
            "import androidx.compose.ui.graphics.Brush\n"
            "import androidx.compose.ui.graphics.Color\n"
            "import androidx.compose.ui.graphics.PathFillType\n"
            "import androidx.compose.ui.graphics.SolidColor\n"
            "import androidx.compose.ui.graphics.vector.ImageVector\n"
            "import androidx.compose.ui.graphics.vector.PathParser\n"
            "import androidx.compose.ui.unit.dp\n\n"
            "internal val AppIcon: ImageVector by lazy {\n"
            f'    ImageVector.Builder("DaGaLi", 24.dp, 24.dp, {viewport}f, {viewport}f).apply {{\n'
            + body
            + "\n    }.build()\n}\n").encode()


def generate():
    geometry = json.loads((ROOT / ART / "monochrome.json").read_text())
    source = artwork(geometry, 1024)
    master = source.copy()
    # Windows, MSIX and the dashboard use a full-bleed tile with the macOS corner ratio.
    icon = tile(geometry, 1024)
    macos = Image.new("RGBA", (1024, 1024))
    macos.alpha_composite(tile(geometry, 896), (64, 64))
    outputs = {ART / "source.png": png(source), ART / "master.png": png(master),
               ART / "icon.png": png(icon), ART / "macos.png": png(macos),
               Path("desktop/resources/dgl.icns"): icns(macos),
               Path("app/src/jvmSharedMain/kotlin/kim/jujin/dglc/ui/AppIcon.kt"): compose_vector(geometry)}
    # Render every ICO entry at its own size instead of shrinking the 1024px tile.
    frames = [tile(geometry, size) for size in ICO_SIZES]
    ico = BytesIO()
    frames[-1].save(ico, format="ICO", sizes=[(size, size) for size in ICO_SIZES],
                    append_images=frames[:-1])
    outputs[Path("desktop/resources/windows/dgl.ico")] = ico.getvalue()
    # MSIX logos keep the exe icon's rounded tile at their unqualified scale-100 sizes,
    # so the package needs no resources.pri.
    for name, size in MSIX_LOGOS:
        outputs[Path(f"desktop/resources/windows/msix/Assets/{name}.png")] = png(tile(geometry, size))

    # Fit both the color and monochrome artwork inside the same ANDROID_ART_DP circle.
    # Measure substantial coverage, preserving antialiasing in the actual output.
    alpha = master.getchannel("A")
    radius = max(((x-511.5)**2 + (y-511.5)**2)**0.5
                 for y in range(1024) for x in range(1024) if alpha.getpixel((x, y)) >= 16)
    factor = (ANDROID_ART_DP / 2 / 108) / (radius / 1024)
    outputs[RES / "drawable/ic_launcher_monochrome.xml"] = vector(geometry, factor)
    outputs[RES / "drawable/ic_launcher_background.xml"] = background_vector()
    # Notification glyph: white alpha only, fitted to 20dp inside a 24dp viewport.
    # Launcher padding belongs to the adaptive mask and must not shrink this glyph.
    bounds = master.getbbox()
    extent = max(bounds[2] - bounds[0], bounds[3] - bounds[1])
    notification_factor = (20 / 24) / (extent / master.width)
    outputs[RES / "drawable/ic_notification.xml"] = vector(geometry, notification_factor, size=24)

    def foreground(size):
        layer = Image.new("RGBA", (size, size))
        scaled = resize(master, round(size * factor))
        offset = (size - scaled.width) // 2
        layer.alpha_composite(scaled, (offset, offset))
        return layer

    for density, size in (("mdpi", 108), ("hdpi", 162), ("xhdpi", 216),
                          ("xxhdpi", 324), ("xxxhdpi", 432)):
        outputs[RES / f"drawable-{density}/ic_launcher_foreground.png"] = png(foreground(size))

    # Real sizes on dark and light surfaces, with nearest-neighbour zoom for small entries.
    sheet = Image.new("RGB", (760, 500), "#20262E")
    draw = ImageDraw.Draw(sheet)
    draw.rectangle((0, 250, 760, 500), fill="#F2F2F2")
    for row, ink in ((0, "white"), (250, "#20262E")):
        x = 16
        for size in (128, 48, 32, 24, 16):
            thumb = tile(geometry, size)
            sheet.paste(thumb, (x, row + 34), thumb)
            draw.text((x, row + 12), f"{size}px", fill=ink)
            if size <= 48:
                zoom = thumb.resize((96, 96), Image.Resampling.NEAREST)
                sheet.paste(zoom, (x, row + 144), zoom)
            x += 150
    outputs[ART / "preview-sizes.png"] = png(sheet)

    sheet = Image.new("RGB", (800, 240), "#20262E")
    draw = ImageDraw.Draw(sheet)
    layer = foreground(432)
    x, y, radius = (value * 4 for value in layer_gradient())
    background = radial(432, (x, y), radius)
    for index, (name, rounded, themed) in enumerate((("Circle", False, False), ("Rounded square", True, False),
                                                    ("Mono circle", False, True), ("Mono rounded", True, True))):
        if themed:
            layer = Image.new("RGBA", (432, 432), LIME)
            layer.putalpha(monochrome(geometry, 432, factor))
        canvas = (Image.new("RGBA", (432, 432), BACKGROUND) if themed else background).copy()
        canvas.alpha_composite(layer)
        # Android's static viewport is the central 72dp of each 108dp layer.
        canvas = canvas.crop((72, 72, 360, 360)).resize((176, 176), Image.Resampling.LANCZOS)
        mask = Image.new("L", canvas.size)
        painter = ImageDraw.Draw(mask)
        if rounded:
            painter.rounded_rectangle((0, 0, 175, 175), radius=40, fill=255)
        else:
            painter.ellipse((0, 0, 175, 175), fill=255)
        sheet.paste(canvas.convert("RGB"), (12 + 200*index, 40), mask)
        draw.text((12 + 200*index, 16), name, fill="white")
    outputs[ART / "preview-android.png"] = png(sheet)
    return outputs


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="Compare generated assets without writing")
    args = parser.parse_args()
    mismatches = []
    for relative, data in generate().items():
        path = ROOT / relative
        if args.check:
            if not path.is_file() or path.read_bytes() != data:
                mismatches.append(str(relative))
        else:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
    if mismatches:
        raise SystemExit("Stale app icons:\n" + "\n".join(mismatches))
    print("App icons match sources." if args.check else "Generated app icons and previews.")


if __name__ == "__main__":
    main()
