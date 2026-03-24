import puppeteer from "puppeteer";
import { mkdir } from "node:fs/promises";
import { join } from "node:path";

const OUTPUT_DIR = join(import.meta.dirname, "references", "katex");

const TEST_EXPRESSIONS = {
  simple_x: "x",
  x_plus_y: "x + y",
  fraction: "\\frac{a}{b}",
  superscript: "x^2",
  subscript: "x_i",
  sub_superscript: "x_i^2",
  nested_fraction: "\\frac{1}{1 + \\frac{1}{x}}",
  quadratic: "\\frac{-b \\pm \\sqrt{b^2 - 4ac}}{2a}",
  sqrt: "\\sqrt{x^2 + y^2}",
  sum_limits: "\\sum_{i=0}^{n} i^2",
  integral: "\\int_0^\\infty e^{-x^2} dx",
  greek: "\\alpha + \\beta = \\gamma",
  product: "\\prod_{k=1}^{n} k",
};

const BASE_HTML = `<!DOCTYPE html>
<html>
<head>
  <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/katex@0.16.11/dist/katex.min.css">
  <script src="https://cdn.jsdelivr.net/npm/katex@0.16.11/dist/katex.min.js"></script>
  <style>
    * { margin: 0; padding: 0; }
    body {
      background: white;
      color: black;
      display: inline-block;
    }
    #math {
      padding: 4px;
      display: inline-block;
    }
  </style>
</head>
<body>
  <div id="math"></div>
</body>
</html>`;

async function main() {
  await mkdir(OUTPUT_DIR, { recursive: true });

  const browser = await puppeteer.launch();
  const page = await browser.newPage();

  await page.setViewport({ width: 800, height: 600, deviceScaleFactor: 2 });

  // Load the page with KaTeX once
  await page.setContent(BASE_HTML, { waitUntil: "networkidle0", timeout: 60000 });

  for (const [name, latex] of Object.entries(TEST_EXPRESSIONS)) {
    // Render each expression by calling katex.render in-page
    await page.evaluate((tex) => {
      const el = document.getElementById("math");
      katex.render(tex, el, { displayMode: true, throwOnError: true });
    }, latex);

    // Small delay for layout
    await new Promise((r) => setTimeout(r, 100));

    const element = await page.$("#math");
    const bbox = await element.boundingBox();

    const padding = 4;
    const clip = {
      x: Math.max(0, bbox.x - padding),
      y: Math.max(0, bbox.y - padding),
      width: bbox.width + padding * 2,
      height: bbox.height + padding * 2,
    };

    const outPath = join(OUTPUT_DIR, `${name}.png`);
    await page.screenshot({ path: outPath, clip });
    console.log(`Saved ${name} -> ${outPath}`);
  }

  await browser.close();
  console.log("Done.");
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
