import ColorThief from "colorthief";
import { promises as fs } from "node:fs";
import os from "node:os";
import path from "node:path";
import { PNG } from "pngjs";

const [, , iconDirArg, outputArg] = process.argv;

if (!iconDirArg || !outputArg) {
  console.error(
    "usage: node scripts/extract-docker-icon-colors.mjs <icon-dir> <output-json>",
  );
  process.exit(1);
}

const iconDir = path.resolve(iconDirArg);
const outputPath = path.resolve(outputArg);
const entries = await fs.readdir(iconDir, { withFileTypes: true });
const colors = {};

for (const entry of entries) {
  if (!entry.isFile() || path.extname(entry.name) !== ".png") {
    continue;
  }

  const iconName = path.basename(entry.name, ".png");
  const iconPath = path.join(iconDir, entry.name);

  try {
    const opaqueIconPath = await writeOpaquePixelsOnlyPng(iconPath, iconName);
    const [r, g, b] = await ColorThief.getColor(opaqueIconPath);
    await fs.rm(opaqueIconPath, { force: true });
    colors[iconName] = rgbToHex(r, g, b);
  } catch (error) {
    console.warn(`warning: failed to extract color for ${entry.name}: ${error.message}`);
  }
}

const sorted = Object.fromEntries(
  Object.entries(colors).sort(([left], [right]) => left.localeCompare(right)),
);

await fs.writeFile(outputPath, `${JSON.stringify(sorted, null, 2)}\n`);

async function writeOpaquePixelsOnlyPng(sourcePath, iconName) {
  const buffer = await fs.readFile(sourcePath);
  const source = PNG.sync.read(buffer);
  const pixels = [];

  for (let offset = 0; offset < source.data.length; offset += 4) {
    const alpha = source.data[offset + 3];
    if (alpha < 16) {
      continue;
    }

    pixels.push([
      source.data[offset],
      source.data[offset + 1],
      source.data[offset + 2],
    ]);
  }

  if (pixels.length === 0) {
    throw new Error("no opaque pixels found");
  }

  const width = Math.ceil(Math.sqrt(pixels.length));
  const height = Math.ceil(pixels.length / width);
  const target = new PNG({ width, height });
  target.data.fill(0);

  pixels.forEach(([r, g, b], index) => {
    const offset = index * 4;
    target.data[offset] = r;
    target.data[offset + 1] = g;
    target.data[offset + 2] = b;
    target.data[offset + 3] = 255;
  });

  const safeName = iconName.replace(/[^a-z0-9-]/gi, "_");
  const outputPath = path.join(os.tmpdir(), `echo-${safeName}-opaque.png`);
  await fs.writeFile(outputPath, PNG.sync.write(target));
  return outputPath;
}

function rgbToHex(r, g, b) {
  return `#${[r, g, b]
    .map((channel) => channel.toString(16).padStart(2, "0"))
    .join("")}`;
}
