import { readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';

export type BenchReportResult = {
  project: string;
  rustJobs?: number | 'auto';
  stats: Record<string, { median: number; min: number; max: number; runs: number } | null>;
  drift: Record<string, string>;
};

export function toolDisplayLabel(tool: string): string {
  return tool;
}

function escapeHtml(unsafe: string): string {
  return unsafe
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#039;");
}

export function renderBenchmarkSvg(results: BenchReportResult[]): string {
  const barHeight = 24;
  const barSpacing = 8;
  const projectSpacing = 24;
  const toolColors: Record<string, string> = {
    'tsc': '#3178c6',
    'tsgo': '#00ADD8',
    'tsgo-singleThreaded': '#00ADD8',
    'surge-ts': '#dea584',
  };

  const toolsOrder = ['tsc', 'tsgo', 'tsgo-singleThreaded', 'surge-ts'];

  // Calculate dimensions
  let maxTime = 0.001; // Avoid divide by 0
  for (const r of results) {
    for (const tool of toolsOrder) {
      if (r.stats[tool]) {
        maxTime = Math.max(maxTime, r.stats[tool]!.median);
      }
    }
  }

  const chartWidth = 800;
  const labelWidth = 250;
  const plotWidth = chartWidth - labelWidth - 40; // 40 is right margin

  let yOffset = 40;
  let svgContent = '';

  for (const r of results) {
    // Project title
    svgContent += `<text x="10" y="${yOffset}" font-family="sans-serif" font-size="14" font-weight="bold" fill="#333">${escapeHtml(r.project)}</text>\n`;
    yOffset += 20;

    for (const tool of toolsOrder) {
      const stats = r.stats[tool];
      if (!stats) continue;

      const drift = tool !== 'tsc' ? r.drift[tool] : '';
      const driftColor = drift === 'exact vs tsc' ? 'green' : 'red';
      const driftText = drift ? ` [${escapeHtml(drift)}]` : '';
      const baseLabel = toolDisplayLabel(tool);
      const toolLabel = tool === 'surge-ts' && r.rustJobs !== undefined ? `${baseLabel} (jobs=${r.rustJobs})` : baseLabel;

      const width = (stats.median / maxTime) * plotWidth;
      const color = toolColors[tool] || '#ccc';

      const tooltip = `${escapeHtml(toolLabel)}: median ${stats.median.toFixed(2)}s, min ${stats.min.toFixed(2)}s, max ${stats.max.toFixed(2)}s, runs ${stats.runs}`;

      svgContent += `<g>
        <title>${tooltip}</title>
        <text x="${labelWidth - 10}" y="${yOffset + 16}" font-family="sans-serif" font-size="12" fill="#555" text-anchor="end">${escapeHtml(toolLabel)}</text>
        <rect x="${labelWidth}" y="${yOffset}" width="${width}" height="${barHeight}" fill="${color}" />
        <text x="${labelWidth + width + 5}" y="${yOffset + 16}" font-family="sans-serif" font-size="12" fill="#333">${stats.median.toFixed(2)}s</text>`;
        
      if (driftText) {
         svgContent += `<text x="${labelWidth + width + 60}" y="${yOffset + 16}" font-family="sans-serif" font-size="12" fill="${driftColor}">${driftText}</text>`;
      }

      svgContent += `</g>\n`;
      yOffset += barHeight + barSpacing;
    }
    yOffset += projectSpacing;
  }

  const chartHeight = yOffset + 20;

  return `<svg xmlns="http://www.w3.org/2000/svg" width="${chartWidth}" height="${chartHeight}" viewBox="0 0 ${chartWidth} ${chartHeight}">
  <rect width="100%" height="100%" fill="#fff" />
  <text x="10" y="20" font-family="sans-serif" font-size="16" font-weight="bold" fill="#000">TypeScript Compilers Benchmark</text>
${svgContent}
</svg>`;
}

export function renderBenchmarkHtml(results: BenchReportResult[]): string {
  const svg = renderBenchmarkSvg(results);
  return `<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <title>TypeScript Compilers Benchmark</title>
  <style>
    body { font-family: sans-serif; padding: 20px; background: #f9f9f9; }
    .container { max-width: 900px; margin: 0 auto; background: #fff; padding: 20px; border-radius: 8px; box-shadow: 0 2px 4px rgba(0,0,0,0.1); }
    .disclaimer { font-size: 0.9em; color: #666; margin-top: 20px; padding-top: 10px; border-top: 1px solid #eee; }
    .drift-warning { color: red; font-weight: bold; }
    .drift-ok { color: green; }
  </style>
</head>
<body>
  <div class="container">
    <h1>TypeScript Compilers Benchmark</h1>
    <p>Visual comparison of compiler timings.</p>
    <div>${svg}</div>
    <div class="disclaimer">
      <strong>Disclaimer:</strong> This is a local-machine-relative regression benchmark. Results are highly dependent on the hardware it was run on. These are not cross-machine or marketing performance claims. Non-exact drift (marked in <span class="drift-warning">red</span>) means diagnostics output differs from baseline tsc.
    </div>
  </div>
</body>
</html>`;
}
