// Compare the pre-change and current production renderers with identical saved art.
// bun scripts/compare-renderer.ts <journal.json> <prefix> <before-url> <after-url>
import { chromium, expect } from "@playwright/test";
import { PNG } from "pngjs";
const [file, prefix, ...urls] = Bun.argv.slice(2);
if (!file || !prefix || urls.length !== 2)
  throw Error("Expected journal, output prefix, before URL, after URL");
const record = await Bun.file(file).text(),
  browser = await chromium.launch({ args: ["--enable-features=WebMCP"] });
const captures: Buffer[][] = [];
try {
  for (const [index, url] of [urls[0], urls[0], urls[1]].entries()) {
    const context = await browser.newContext({
      viewport: { width: 1300, height: 1000 },
      deviceScaleFactor: 2,
    });
    const page = await context.newPage();
    await page.goto(url);
    await expect(page.getByTestId("agent-status")).toContainText("Agent ready");
    await page.evaluate(async (record) => {
      await new Promise<void>((resolve, reject) => {
        const open = indexedDB.open("agent-whiteboard", 1);
        open.onsuccess = () => {
          const db = open.result,
            tx = db.transaction("boards", "readwrite");
          tx.objectStore("boards").put(record, "local");
          tx.oncomplete = () => {
            db.close();
            resolve();
          };
          tx.onerror = () => reject(tx.error);
        };
        open.onerror = () => reject(open.error);
      });
    }, record);
    await page.reload();
    await expect(page.getByTestId("agent-status")).toContainText("Agent ready");
    const images: Buffer[] = [];
    async function capture(bounds: object) {
      const result = await page.webmcp.callTool("get_view", { bounds, maxSize: 1050 });
      if (result.isError) throw Error(JSON.stringify(result));
      images.push(
        Buffer.from(
          result.content.find((c: { type: string }) => c.type === "image").data,
          "base64",
        ),
      );
    }
    await capture({ x: 40, y: 300, width: 1160, height: 1100 });
    await capture({ x: 0, y: 0, width: 4096, height: 2304 });
    const result = await page.webmcp.callTool("apply_operations", {
      commandId: "affine-raster-parity",
      operations: [
        {
          op: "create",
          id: "affine-path",
          drawing: {
            geometry: {
              kind: "path",
              segments: [
                { op: "M", x: 0, y: 0 },
                { op: "L", x: 300, y: 0 },
                { op: "L", x: 300, y: 200 },
                { op: "L", x: 0, y: 200 },
                { op: "Z" },
              ],
            },
            transform: [2, 0.4, 0.8, 3, 4500, -1000],
            style: { stroke: "#4083bf", fill: "#e4a13c", width: 70, opacity: 0.45 },
          },
        },
        {
          op: "create",
          id: "wide-text",
          drawing: {
            geometry: { kind: "text", x: 0, y: 0, text: "WWWgyp\nÅff", size: 120 },
            transform: [-1, 0.3, 0.6, 1.2, 5400, -1200],
            style: { stroke: "#8062ab", opacity: 0.55 },
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
                { x: 350, y: 250 },
              ],
            },
            transform: [1, 0.3, 0.7, 1.2, 4600, -900],
            clip: {
              segments: [
                { op: "M", x: 0, y: 0 },
                { op: "L", x: 300, y: 0 },
                { op: "L", x: 300, y: 200 },
                { op: "L", x: 0, y: 200 },
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
      ],
    });
    if (result.isError) throw Error(JSON.stringify(result));
    const sequence = JSON.parse(result.content[0].text).receipt.sequence;
    const erased = await page.webmcp.callTool("apply_operations", {
      commandId: "affine-mask-parity",
      operations: [
        {
          op: "erase",
          targets: [{ id: "affine-path", expectedVersion: sequence }],
          points: [
            { x: 4700, y: -700 },
            { x: 5000, y: -400 },
          ],
          radius: 60,
        },
      ],
    });
    if (erased.isError) throw Error(JSON.stringify(erased));
    await capture({ x: 4300, y: -1400, width: 1500, height: 1400 });
    for (const view of [
      { cx: 620, cy: 820, zoom: 2.15 },
      { cx: 670, cy: 840, zoom: 1.7 },
    ]) {
      await page.webmcp.callTool("set_viewport", view);
      await page.evaluate(() => document.fonts.ready);
      images.push(
        Buffer.from(
          (
            await page
              .locator("canvas")
              .evaluate((canvas) => (canvas as HTMLCanvasElement).toDataURL())
          ).split(",")[1],
          "base64",
        ),
      );
    }
    for (let i = 0; i < images.length; i++)
      await Bun.write(prefix + "-" + index + "-" + i + ".png", images[i]);
    captures.push(images);
    await context.close();
  }
  const compare = (other: Buffer[]) =>
    captures[0].map((buffer, index) => {
      const a = PNG.sync.read(buffer),
        b = PNG.sync.read(other[index]);
      if (a.width !== b.width || a.height !== b.height) throw Error("Different image bounds");
      let changed = 0,
        beyondOne = 0,
        beyondTwo = 0,
        max = 0,
        total = 0;
      for (let i = 0; i < a.data.length; i += 4) {
        let d = 0;
        for (let c = 0; c < 4; c++) d = Math.max(d, Math.abs(a.data[i + c] - b.data[i + c]));
        if (d) changed++;
        if (d > 1) beyondOne++;
        if (d > 2) beyondTwo++;
        max = Math.max(max, d);
        total += d;
      }
      return {
        index,
        pixels: a.width * a.height,
        changed,
        beyondOne,
        beyondTwo,
        max,
        mean: total / (a.width * a.height),
      };
    });
  const control = compare(captures[1]),
    after = compare(captures[2]);
  console.log(JSON.stringify({ control, after }, null, 2));
  await Bun.write(prefix + ".json", JSON.stringify({ control, after }, null, 2));
  if (control.some((r) => r.changed))
    throw Error("The unchanged-build control is not pixel deterministic");
  if (after.some((r) => r.changed))
    throw Error("The optimized drawing differs from the original renderer");
} finally {
  await browser.close();
}
