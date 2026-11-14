import { expect, test, type Page } from "@playwright/test";
import { PNG } from "pngjs";
import { apply, changedPixels, ready } from "./helpers";
import type { Operation, Paint } from "../protocol";

type Work = { paths: number; fills: number; allocatedPixels: number };
async function work(page: Page): Promise<Work> {
  return page.evaluate(() => ({ ...Reflect.get(window, "__rendererWork") }));
}
async function observe(page: Page) {
  await page.addInitScript(() => {
    const work = { paths: 0, fills: 0, allocatedPixels: 0 };
    Object.assign(window, { __rendererWork: work });
    const Path = window.Path2D;
    window.Path2D = new Proxy(Path, {
      construct(target, args) {
        work.paths++;
        return Reflect.construct(target, args);
      },
    });
    for (const name of ["fill", "stroke"] as const) {
      const original = CanvasRenderingContext2D.prototype[name];
      Object.defineProperty(CanvasRenderingContext2D.prototype, name, {
        value: function (...args: unknown[]) {
          work.fills++;
          return Reflect.apply(original, this, args);
        },
      });
    }
    for (const name of ["width", "height"]) {
      const descriptor = Object.getOwnPropertyDescriptor(HTMLCanvasElement.prototype, name)!;
      Object.defineProperty(HTMLCanvasElement.prototype, name, {
        ...descriptor,
        set(value) {
          work.allocatedPixels += Number(value) * (name === "width" ? this.height : this.width);
          descriptor.set!.call(this, value);
        },
      });
    }
  });
}

test("large boards retain paths, bound compositing work, and reuse finished art during pointer previews", async ({
  page,
}) => {
  await observe(page);
  await ready(page, process.env.WHITEBOARD_REFERENCE_URL ?? "./");
  const operations: Operation[] = Array.from({ length: 700 }, (_, i) => {
    const x = 100 + (i % 35) * 100,
      y = 100 + Math.floor(i / 35) * 90;
    return {
      op: "create",
      id: "mark-" + i,
      drawing: {
        geometry: {
          kind: "path",
          segments: [
            { op: "M", x, y },
            { op: "L", x: x + 20, y },
            { op: "L", x: x + 20, y: y + 20 },
            { op: "L", x, y: y + 20 },
            { op: "Z" },
          ],
        },
        style: { stroke: "#4083bf", fill: "#4083bf", opacity: 0.35, width: 2 },
      },
    };
  });
  for (let i = 0; i < operations.length; i += 250)
    expect((await apply(page, operations.slice(i, i + 250))).isError).not.toBe(true);
  await page.webmcp.callTool("set_viewport", { cx: 2048, cy: 1152, zoom: 1 });
  const before = await work(page);
  await page.webmcp.callTool("set_viewport", { cx: 2052, cy: 1152, zoom: 1 });
  const after = await work(page);
  expect(
    after.paths - before.paths,
    "Panning must reuse paths on boards larger than the old 500-entry limit",
  ).toBe(0);
  const canvas = page.locator("canvas"),
    pixels = await canvas.evaluate(
      (canvas) => (canvas as HTMLCanvasElement).width * (canvas as HTMLCanvasElement).height,
    );
  expect(
    after.allocatedPixels - before.allocatedPixels,
    "Small translucent marks must not allocate a whole viewport each",
  ).toBeLessThan(pixels * 8);
  const bounds = (await canvas.boundingBox())!;
  const previewBefore = await work(page);
  await page.mouse.move(bounds.x + bounds.width * 0.3, bounds.y + bounds.height * 0.8);
  await page.mouse.down();
  await page.mouse.move(bounds.x + bounds.width * 0.65, bounds.y + bounds.height * 0.85, {
    steps: 6,
  });
  await expect(canvas).toHaveAttribute("data-drafting", "true");
  const previewAfter = await work(page);
  expect(
    previewAfter.fills - previewBefore.fills,
    "Pointer movement must paint the changing mark, not all 700 finished marks",
  ).toBeLessThan(30);
  await page.mouse.up();
  await expect(canvas).toHaveAttribute("data-sequence", "4");
});

test("bounded compositing preserves scaled strokes, soft clips and eraser masks", async ({
  page,
}) => {
  await ready(page);
  const made = await apply(page, [
    {
      op: "create",
      id: "scaled-path",
      drawing: {
        geometry: {
          kind: "path",
          segments: [
            { op: "M", x: 0, y: 0 },
            { op: "L", x: 100, y: 0 },
            { op: "L", x: 100, y: 90 },
            { op: "L", x: 0, y: 90 },
            { op: "Z" },
          ],
        },
        transform: [2, 1.2, 1, 3, 300, 400],
        style: { stroke: "#4083bf", fill: "#e4a13c", width: 60, opacity: 0.4 },
      },
    },
    {
      op: "create",
      id: "soft-clip",
      drawing: {
        geometry: {
          kind: "stroke",
          points: [
            { x: 0, y: 0 },
            { x: 200, y: 200 },
          ],
        },
        transform: [1, 0.2, 0.5, 0.8, 450, 350],
        clip: {
          segments: [
            { op: "M", x: 0, y: 0 },
            { op: "L", x: 150, y: 0 },
            { op: "L", x: 150, y: 150 },
            { op: "L", x: 0, y: 150 },
            { op: "Z" },
          ],
        },
        style: {
          stroke: "#d97783",
          width: 100,
          opacity: 0.6,
          brush: { kind: "soft", softness: 0.8 },
        },
      },
    },
  ]);
  expect(made.isError).not.toBe(true);
  await apply(page, [
    {
      op: "erase",
      targets: [{ id: "scaled-path", expectedVersion: 1 }],
      points: [{ x: 450, y: 600 }],
      radius: 40,
    },
  ]);
  const exported = await page.webmcp.callTool("export_board", { format: "svg" });
  expect(exported.isError).not.toBe(true);
  const svg = JSON.parse(exported.content[0].text).data;
  const data = await page.evaluate(async (svg) => {
    await document.fonts.ready;
    const img = new Image(),
      url = URL.createObjectURL(new Blob([svg], { type: "image/svg+xml" }));
    try {
      img.src = url;
      await img.decode();
      const canvas = document.createElement("canvas");
      canvas.width = 1000;
      canvas.height = 1000;
      canvas.getContext("2d")!.drawImage(img, 0, 0);
      return canvas.toDataURL();
    } finally {
      URL.revokeObjectURL(url);
    }
  }, svg);
  const reference = Buffer.from(data.split(",")[1], "base64");
  const direct = await page.webmcp.callTool("get_view", {
    bounds: { x: 0, y: 0, width: 1000, height: 1000 },
    maxSize: 1000,
  });
  const directPng = Buffer.from(
    direct.content.find((b: { type: string }) => b.type === "image").data,
    "base64",
  );
  expect(changedPixels(reference, directPng)).toBeLessThan(1200);
});

test("translucent text keeps wide glyphs beyond the selection estimate", async ({ page }) => {
  await ready(page);
  await apply(page, [
    {
      op: "create",
      id: "wide",
      drawing: {
        geometry: { kind: "text", x: 100, y: 250, text: "WWW", size: 160 },
        style: { stroke: "#8062ab", opacity: 0.55 },
      },
    },
  ]);
  const result = await page.webmcp.callTool("get_view", {
    bounds: { x: 0, y: 0, width: 1000, height: 1000 },
    maxSize: 1000,
  });
  expect(result.isError).not.toBe(true);
  const png = PNG.sync.read(
    Buffer.from(result.content.find((b: { type: string }) => b.type === "image").data, "base64"),
  );
  let beyond = 0;
  for (let y = 100; y < 270; y++)
    for (let x = 460; x < 650; x++) {
      const i = (y * png.width + x) * 4;
      if (png.data[i + 1] < 230 && png.data[i + 2] > png.data[i + 1] + 10) beyond++;
    }
  expect(beyond).toBeGreaterThan(500);
});

test("cached selection previews remove the original and preserve foreground layer order", async ({
  page,
}) => {
  await ready(page);
  const rect = (id: string, color: string, x: number, w: number): Operation => ({
    op: "create",
    id,
    drawing: {
      geometry: {
        kind: "path",
        segments: [
          { op: "M", x, y: 800 },
          { op: "L", x: x + w, y: 800 },
          { op: "L", x: x + w, y: 1300 },
          { op: "L", x, y: 1300 },
          { op: "Z" },
        ],
      },
      style: { stroke: color, fill: color, width: 2 },
    },
  });
  await apply(page, [rect("lower", "#ff0000", 1000, 700), rect("upper", "#4083bf", 1800, 500)]);
  await page.getByRole("button", { name: "Select", exact: true }).click();
  const canvas = page.locator("canvas"),
    bounds = (await canvas.boundingBox())!,
    scale = Math.min(bounds.width / 4096, bounds.height / 2304);
  const screen = (x: number, y: number) => ({
    x: bounds.x + bounds.width / 2 + (x - 2048) * scale,
    y: bounds.y + bounds.height / 2 + (y - 1152) * scale,
  });
  const start = screen(1200, 1050),
    end = screen(2000, 1050);
  await page.mouse.move(start.x, start.y);
  await page.mouse.down();
  await page.mouse.move(end.x, end.y, { steps: 6 });
  await expect(canvas).toHaveAttribute("data-drafting", "true");
  const colors = await canvas.evaluate(
    (element, positions) => {
      const canvas = element as HTMLCanvasElement,
        ctx = canvas.getContext("2d")!,
        r = canvas.getBoundingClientRect();
      return positions.map((p) =>
        [
          ...ctx.getImageData(
            Math.round(((p.x - r.x) * canvas.width) / r.width),
            Math.round(((p.y - r.y) * canvas.height) / r.height),
            1,
            1,
          ).data,
        ].slice(0, 3),
      );
    },
    [start, end],
  );
  expect(Math.min(...colors[0])).toBeGreaterThan(245);
  expect(colors[1]).toEqual([64, 131, 191]);
  await page.mouse.up();
  await expect(canvas).toHaveAttribute("data-sequence", "2");
});

test("gradient isolation preserves the native viewport-aligned raster exactly", async ({
  page,
}) => {
  await ready(page);
  const paint: Paint = {
    kind: "radial",
    cx: 300,
    cy: 300,
    radius: 500,
    stops: [
      { offset: 0, color: "#fff5cf", opacity: 0.25 },
      { offset: 0.4, color: "#ffe795", opacity: 0.04 },
      { offset: 1, color: "#94360d", opacity: 0.65 },
    ],
  };
  await apply(page, [
    {
      op: "create",
      id: "gradient-origin",
      drawing: {
        geometry: {
          kind: "path",
          segments: [
            { op: "M", x: 120, y: 150 },
            { op: "L", x: 700, y: 150 },
            { op: "L", x: 700, y: 700 },
            { op: "L", x: 120, y: 700 },
            { op: "Z" },
          ],
        },
        style: { stroke: paint, fill: paint, width: 2, opacity: 0.75 },
      },
    },
  ]);
  const expected = await page.evaluate(() => {
    const canvas = document.createElement("canvas"),
      layer = document.createElement("canvas");
    canvas.width = layer.width = 1000;
    canvas.height = layer.height = 1000;
    const ctx = canvas.getContext("2d")!,
      ink = layer.getContext("2d")!,
      gradient = ink.createRadialGradient(300, 300, 0, 300, 300, 500);
    gradient.addColorStop(0, "rgba(255,245,207,0.25)");
    gradient.addColorStop(0.4, "rgba(255,231,149,0.04)");
    gradient.addColorStop(1, "rgba(148,54,13,0.65)");
    ink.fillStyle = ink.strokeStyle = gradient;
    ink.lineWidth = 2;
    ink.lineJoin = ink.lineCap = "round";
    const shape = new Path2D("M120 150 L700 150 L700 700 L120 700 Z");
    ink.fill(shape);
    ink.stroke(shape);
    ctx.fillStyle = "#ffffff";
    ctx.fillRect(0, 0, 1000, 1000);
    ctx.globalAlpha = 0.75;
    ctx.drawImage(layer, 0, 0);
    return canvas.toDataURL();
  });
  const actual = await page.webmcp.callTool("get_view", {
    bounds: { x: 0, y: 0, width: 1000, height: 1000 },
    maxSize: 1000,
  });
  expect(actual.isError).not.toBe(true);
  const before = PNG.sync.read(Buffer.from(expected.split(",")[1], "base64")),
    after = PNG.sync.read(
      Buffer.from(actual.content.find((b: { type: string }) => b.type === "image").data, "base64"),
    );
  expect(
    after.data.equals(before.data),
    "Isolating a non-origin gradient must retain the browser’s exact pixel colors",
  ).toBe(true);
});
