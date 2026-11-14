// Profile a production build with a local journal copy. Never changes the source board.
// bun scripts/profile-whiteboard.ts <journal.json> <output-prefix> [url]
import { chromium, expect } from "@playwright/test";
const [recordPath, prefix, url = "http://127.0.0.1:4174/agent-whiteboard/"] = Bun.argv.slice(2);
if (!recordPath || !prefix)
  throw Error("Usage: bun scripts/profile-whiteboard.ts <journal.json> <output-prefix> [url]");
const record = await Bun.file(recordPath).text();
const browser = await chromium.launch({ args: ["--enable-features=WebMCP"] });
try {
  const page = await browser.newPage({
    viewport: { width: 1300, height: 1000 },
    deviceScaleFactor: 2,
  });
  await page.goto(url);
  await expect(page.getByTestId("agent-status")).toContainText("Agent ready");
  await page.evaluate(async (record) => {
    await new Promise<void>((resolve, reject) => {
      const open = indexedDB.open("agent-whiteboard", 1);
      open.onerror = () => reject(open.error);
      open.onsuccess = () => {
        const db = open.result;
        const tx = db.transaction("boards", "readwrite");
        tx.objectStore("boards").put(record, "local");
        tx.oncomplete = () => {
          db.close();
          resolve();
        };
        tx.onerror = () => reject(tx.error);
      };
    });
  }, record);
  await page.addInitScript(() => {
    const counters = {
      paths: 0,
      resizes: 0,
      composites: 0,
      fills: 0,
      frames: 0,
      longTasks: [] as number[],
    };
    Object.assign(window, { __profile: counters });
    const Path = window.Path2D;
    window.Path2D = new Proxy(Path, {
      construct(target, args) {
        counters.paths++;
        return Reflect.construct(target, args);
      },
    });
    for (const name of ["width", "height"]) {
      const descriptor = Object.getOwnPropertyDescriptor(HTMLCanvasElement.prototype, name)!;
      Object.defineProperty(HTMLCanvasElement.prototype, name, {
        ...descriptor,
        set(value) {
          counters.resizes++;
          descriptor.set!.call(this, value);
        },
      });
    }
    for (const name of ["drawImage", "fill", "stroke"] as const) {
      const original = CanvasRenderingContext2D.prototype[name];
      Object.defineProperty(CanvasRenderingContext2D.prototype, name, {
        value: function (...args: unknown[]) {
          if (name === "drawImage") counters.composites++;
          else counters.fills++;
          return Reflect.apply(original, this, args);
        },
      });
    }
    const frame = requestAnimationFrame;
    window.requestAnimationFrame = (callback) =>
      frame((time) => {
        counters.frames++;
        callback(time);
      });
    new PerformanceObserver((list) => {
      for (const entry of list.getEntries()) counters.longTasks.push(entry.duration);
    }).observe({ type: "longtask", buffered: true });
  });
  const start = performance.now();
  await page.reload();
  await expect(page.getByTestId("agent-status")).toContainText("Agent ready");
  const loadMs = performance.now() - start;
  const cdp = await page.context().newCDPSession(page);
  await cdp.send("Performance.enable");
  await cdp.send("Profiler.enable");
  const metrics = async () =>
    Object.fromEntries(
      (await cdp.send("Performance.getMetrics")).metrics.map((m) => [m.name, m.value]),
    );
  const counts = () => page.evaluate(() => structuredClone(Reflect.get(window, "__profile")));
  const results: Record<string, unknown> = {
    elements: JSON.parse(record).elements.length,
    loadMs,
    browser: browser.version(),
    viewport: { width: 1300, height: 1000, deviceScaleFactor: 2 },
    fixtureSha256: new Bun.CryptoHasher("sha256").update(record).digest("hex"),
  };
  async function sample(name: string, action: () => Promise<unknown>) {
    const before = await metrics(),
      countBefore = await counts();
    await cdp.send("Profiler.start");
    const start = performance.now();
    await action();
    const wallMs = performance.now() - start;
    const profile = await cdp.send("Profiler.stop");
    const after = await metrics(),
      countAfter = await counts();
    await Bun.write(prefix + "-" + name + ".cpuprofile", JSON.stringify(profile.profile));
    results[name] = {
      wallMs,
      scriptMs: (after.ScriptDuration - before.ScriptDuration) * 1000,
      taskMs: (after.TaskDuration - before.TaskDuration) * 1000,
      heapMiB: after.JSHeapUsedSize / 2 ** 20,
      paths: countAfter.paths - countBefore.paths,
      resizes: countAfter.resizes - countBefore.resizes,
      composites: countAfter.composites - countBefore.composites,
      frames: countAfter.frames - countBefore.frames,
      longTasks: countAfter.longTasks.slice(countBefore.longTasks.length),
    };
    console.log(name, JSON.stringify(results[name]));
  }
  await sample("idle", () => page.waitForTimeout(2000));
  await page.webmcp.callTool("set_viewport", { cx: 620, cy: 820, zoom: 2.15 });
  await sample("pan", async () => {
    for (let i = 0; i < 3; i++)
      await page.webmcp.callTool("set_viewport", { cx: 620 + i * 4, cy: 820, zoom: 2.15 });
  });
  const bounds = (await page.locator("canvas").boundingBox())!;
  await sample("draw", async () => {
    await page.mouse.move(bounds.x + bounds.width * 0.2, bounds.y + bounds.height * 0.3);
    await page.mouse.down();
    await page.mouse.move(bounds.x + bounds.width * 0.7, bounds.y + bounds.height * 0.35, {
      steps: 6,
    });
    await page.mouse.up();
    await expect(page.locator("canvas")).toHaveAttribute("data-drafting", "false");
    await expect(page.locator("canvas")).toHaveAttribute(
      "data-sequence",
      String(JSON.parse(record).sequence + 1),
    );
  });
  await sample("replay", () =>
    page.webmcp.callTool("replay_drawing", { action: "start", durationMs: 2000 }),
  );
  await page.screenshot({ path: prefix + ".png" });
  await cdp.send("HeapProfiler.collectGarbage");
  results.retainedHeapMiB = (await metrics()).JSHeapUsedSize / 2 ** 20;
  await Bun.write(prefix + ".json", JSON.stringify(results, null, 2));
  console.log("Saved " + prefix + ".json");
} finally {
  await browser.close();
}
