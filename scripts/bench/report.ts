export type BenchRunStats = { median: number; min: number; max: number; runs: number };

export type BenchMemoryStats = {
  medianBytes: number;
  minBytes: number;
  maxBytes: number;
  runs: number;
  source: string;
};

export type BenchReportResult = {
  project: string;
  rustJobs?: number | 'auto';
  stats: Record<string, BenchRunStats | null>;
  memory?: Record<string, BenchMemoryStats | null>;
  drift: Record<string, string>;
};

export type BenchReportMeta = {
  timestamp?: string;
  gitCommit?: string;
  gitBranch?: string;
  platform?: string;
  cpu?: string;
  cores?: number;
  nodeVersion?: string;
  iterations?: number;
  warmup?: number;
  tscVersion?: string;
  tsgoVersion?: string;
};

export type BenchReportDocument = {
  meta?: BenchReportMeta;
  results: BenchReportResult[];
};

export type BenchReportInput = BenchReportDocument | BenchReportResult[];

export type BenchSvgPanels = 'both' | 'time' | 'memory';

/// Accepts both the current `{ meta, results }` document and the legacy
/// bare-array JSON emitted before run metadata was recorded.
export function normalizeBenchReport(data: unknown): BenchReportDocument {
  if (Array.isArray(data)) {
    return { results: data as BenchReportResult[] };
  }
  if (data && typeof data === 'object' && Array.isArray((data as BenchReportDocument).results)) {
    const doc = data as BenchReportDocument;
    return { meta: doc.meta, results: doc.results };
  }
  throw new Error('Unrecognized benchmark report shape: expected an array or { meta, results }');
}

/// `tsc` is the legacy JS compiler (TypeScript 6.x baseline) and `tsgo` is
/// the native TypeScript 7 compiler; label them so readers don't mistake the
/// slow baseline for current TypeScript.
const TOOL_LABELS: Record<string, string> = {
  'tsc': 'tsc (TS 6)',
  'tsgo': 'tsgo (TS 7)',
  'tsgo-singleThreaded': 'tsgo-singleThreaded (TS 7)',
};

export function toolDisplayLabel(tool: string): string {
  return TOOL_LABELS[tool] ?? tool;
}

const TOOLS_ORDER = ['tsc', 'tsgo', 'tsgo-singleThreaded', 'surge-ts'];

const TOOL_COLORS: Record<string, string> = {
  'tsc': '#3178c6',
  'tsgo': '#00add8',
  'tsgo-singleThreaded': '#73cfe3',
  'surge-ts': '#de7a4a',
};

type DriftStyle = { bg: string; fg: string };

const DRIFT_STYLES: Record<string, DriftStyle> = {
  'baseline': { bg: '#eceff1', fg: '#546e7a' },
  'exact vs tsc': { bg: '#e6f4ea', fg: '#137333' },
  'known delta': { bg: '#fef7e0', fg: '#b06000' },
  'parse failed': { bg: '#fce8e6', fg: '#c5221f' },
};

const DRIFT_FALLBACK: DriftStyle = { bg: '#eceff1', fg: '#546e7a' };

function driftStyle(drift: string): DriftStyle {
  return DRIFT_STYLES[drift] ?? DRIFT_FALLBACK;
}

function escapeHtml(unsafe: string): string {
  return unsafe
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#039;");
}

export function toolLabelForResult(tool: string, result: BenchReportResult): string {
  const base = toolDisplayLabel(tool);
  if (tool === 'surge-ts' && result.rustJobs !== undefined) {
    return `${base} (jobs=${result.rustJobs})`;
  }
  return base;
}

/// Median-vs-median speed ratio relative to the tsc baseline: >1 means the
/// tool is faster than tsc. Null when either median is missing or zero.
export function speedupVsTsc(result: BenchReportResult, tool: string): number | null {
  if (tool === 'tsc') return null;
  const base = result.stats['tsc'];
  const own = result.stats[tool];
  if (!base || !own || base.median <= 0 || own.median <= 0) return null;
  return base.median / own.median;
}

/// Median peak-RSS ratio relative to the tsc baseline: <1 means the tool uses
/// less memory than tsc. Null when either sample is missing or zero.
export function memoryRatioVsTsc(result: BenchReportResult, tool: string): number | null {
  if (tool === 'tsc') return null;
  const base = result.memory?.['tsc'];
  const own = result.memory?.[tool];
  if (!base || !own || base.medianBytes <= 0 || own.medianBytes <= 0) return null;
  return own.medianBytes / base.medianBytes;
}

export function formatSpeedup(ratio: number): string {
  const digits = ratio >= 10 ? 1 : 2;
  return `${ratio.toFixed(digits)}× vs tsc`;
}

export function formatMemoryRatio(ratio: number): string {
  const digits = ratio >= 10 ? 1 : 2;
  return `${ratio.toFixed(digits)}× of tsc`;
}

function formatSecondsShort(seconds: number): string {
  if (seconds >= 100) return `${seconds.toFixed(0)}s`;
  if (seconds >= 10) return `${seconds.toFixed(1)}s`;
  return `${seconds.toFixed(2)}s`;
}

export function formatBytes(bytes: number): string {
  const mb = bytes / (1024 * 1024);
  if (mb >= 1024) return `${(mb / 1024).toFixed(2)} GB`;
  if (mb >= 10) return `${mb.toFixed(0)} MB`;
  return `${mb.toFixed(1)} MB`;
}

function formatMbTick(mb: number): string {
  if (mb >= 1024) {
    const gb = mb / 1024;
    return `${parseFloat(gb.toPrecision(4))}GB`;
  }
  return `${parseFloat(mb.toPrecision(4))}MB`;
}

/// Round the axis maximum up to a "nice" tick step so gridline labels land on
/// clean values (1/2/2.5/5 x 10^k).
export function niceAxisScale(maxValue: number, targetTicks = 5): { max: number; step: number } {
  const safeMax = maxValue > 0 ? maxValue : 1;
  const rawStep = safeMax / targetTicks;
  const magnitude = 10 ** Math.floor(Math.log10(rawStep));
  const residual = rawStep / magnitude;
  let niceResidual: number;
  if (residual <= 1) niceResidual = 1;
  else if (residual <= 2) niceResidual = 2;
  else if (residual <= 2.5) niceResidual = 2.5;
  else if (residual <= 5) niceResidual = 5;
  else niceResidual = 10;
  const step = niceResidual * magnitude;
  return { max: Math.ceil(safeMax / step) * step, step };
}

function metaSummaryParts(meta: BenchReportMeta | undefined): string[] {
  if (!meta) return [];
  const parts: string[] = [];
  if (meta.timestamp) parts.push(meta.timestamp.replace('T', ' ').replace(/\.\d+Z$/, 'Z'));
  if (meta.cpu) parts.push(meta.cores ? `${meta.cpu} (${meta.cores} cores)` : meta.cpu);
  if (meta.platform) parts.push(meta.platform);
  if (meta.iterations !== undefined) {
    const warmup = meta.warmup !== undefined ? `, warmup ${meta.warmup}` : '';
    parts.push(`${meta.iterations} iterations${warmup}`);
  }
  if (meta.tscVersion) parts.push(`tsc@${meta.tscVersion}`);
  if (meta.tsgoVersion) parts.push(`tsgo@${meta.tsgoVersion}`);
  if (meta.gitCommit) {
    parts.push(meta.gitBranch ? `${meta.gitBranch}@${meta.gitCommit}` : meta.gitCommit);
  }
  return parts;
}

export function hasMemoryData(results: BenchReportResult[]): boolean {
  return results.some((r) =>
    Object.values(r.memory ?? {}).some((entry) => entry !== null && entry !== undefined),
  );
}

type PanelRow = {
  label: string;
  color: string;
  value: number;
  min: number;
  max: number;
  valueText: string;
  ratioText: string | null;
  ratioGood: boolean;
  drift: string | null;
  tooltip: string;
};

type PanelGroup = { project: string; rows: PanelRow[] };

type Panel = {
  title: string;
  groups: PanelGroup[];
  maxValue: number;
  tickFormat: (value: number) => string;
};

function buildTimePanel(results: BenchReportResult[]): Panel | null {
  const groups: PanelGroup[] = [];
  let maxValue = 0;
  for (const r of results) {
    const rows: PanelRow[] = [];
    for (const tool of TOOLS_ORDER) {
      const stats = r.stats[tool];
      if (!stats) continue;
      maxValue = Math.max(maxValue, stats.max, stats.median);
      const label = toolLabelForResult(tool, r);
      const speedup = speedupVsTsc(r, tool);
      const drift = r.drift[tool];
      rows.push({
        label,
        color: TOOL_COLORS[tool] ?? '#9aa5b1',
        value: stats.median,
        min: stats.min,
        max: stats.max,
        valueText: formatSecondsShort(stats.median),
        ratioText: speedup === null ? null : formatSpeedup(speedup),
        ratioGood: speedup !== null && speedup >= 1,
        drift: drift && drift !== 'skipped' && tool !== 'tsc' ? drift : null,
        tooltip: `${label}: median ${stats.median.toFixed(2)}s, min ${stats.min.toFixed(2)}s, max ${stats.max.toFixed(2)}s, runs ${stats.runs}`,
      });
    }
    if (rows.length > 0) groups.push({ project: r.project, rows });
  }
  if (groups.length === 0) return null;
  return {
    title: 'Wall time (median, lower is better)',
    groups,
    maxValue,
    tickFormat: (v) => `${parseFloat(v.toPrecision(12))}s`,
  };
}

function buildMemoryPanel(results: BenchReportResult[]): Panel | null {
  const mb = (bytes: number) => bytes / (1024 * 1024);
  const groups: PanelGroup[] = [];
  let maxValue = 0;
  for (const r of results) {
    const rows: PanelRow[] = [];
    for (const tool of TOOLS_ORDER) {
      const memory = r.memory?.[tool];
      if (!memory) continue;
      maxValue = Math.max(maxValue, mb(memory.maxBytes), mb(memory.medianBytes));
      const label = toolLabelForResult(tool, r);
      const ratio = memoryRatioVsTsc(r, tool);
      rows.push({
        label,
        color: TOOL_COLORS[tool] ?? '#9aa5b1',
        value: mb(memory.medianBytes),
        min: mb(memory.minBytes),
        max: mb(memory.maxBytes),
        valueText: formatBytes(memory.medianBytes),
        ratioText: ratio === null ? null : formatMemoryRatio(ratio),
        ratioGood: ratio !== null && ratio <= 1,
        drift: null,
        tooltip: `${label}: peak memory median ${formatBytes(memory.medianBytes)}, min ${formatBytes(memory.minBytes)}, max ${formatBytes(memory.maxBytes)}, runs ${memory.runs} (${memory.source})`,
      });
    }
    if (rows.length > 0) groups.push({ project: r.project, rows });
  }
  if (groups.length === 0) return null;
  return {
    title: 'Peak memory (median, lower is better)',
    groups,
    maxValue,
    tickFormat: formatMbTick,
  };
}

const CHART_WIDTH = 960;
const LABEL_WIDTH = 218;
const PLOT_X = LABEL_WIDTH + 16;
const PLOT_WIDTH = 420;
const ANNOTATION_X = PLOT_X + PLOT_WIDTH + 14;
const BAR_HEIGHT = 20;
const ROW_HEIGHT = 30;
const PROJECT_SPACING = 26;

function renderPanel(panel: Panel, yStart: number): { svg: string; yEnd: number } {
  const axis = niceAxisScale(panel.maxValue);
  const xFor = (value: number) => PLOT_X + (value / axis.max) * PLOT_WIDTH;

  let svg = `<text x="16" y="${yStart + 12}" font-size="12" font-weight="600" fill="#486581" letter-spacing="0.04em">${escapeHtml(panel.title.toUpperCase())}</text>\n`;
  const ticksY = yStart + 32;
  let yOffset = yStart + 44;

  let body = '';
  for (const group of panel.groups) {
    body += `<text x="16" y="${yOffset + 4}" font-size="14" font-weight="600" fill="#1f2933">${escapeHtml(group.project)}</text>\n`;
    yOffset += 14;

    for (const row of group.rows) {
      const barY = yOffset + (ROW_HEIGHT - BAR_HEIGHT) / 2;
      const centerY = barY + BAR_HEIGHT / 2;
      const barW = Math.max(1, xFor(row.value) - PLOT_X);

      body += `<g>
    <title>${escapeHtml(row.tooltip)}</title>
    <text x="${LABEL_WIDTH}" y="${centerY + 4}" font-size="12" fill="#3e4c59" text-anchor="end">${escapeHtml(row.label)}</text>
    <rect x="${PLOT_X}" y="${barY}" width="${barW.toFixed(1)}" height="${BAR_HEIGHT}" rx="3" fill="${row.color}" fill-opacity="0.9" />`;

      // min–max whisker across the observed run spread
      if (row.max > row.min) {
        const minX = xFor(row.min).toFixed(1);
        const maxX = xFor(Math.min(row.max, axis.max)).toFixed(1);
        body += `
    <line x1="${minX}" y1="${centerY}" x2="${maxX}" y2="${centerY}" stroke="#1f2933" stroke-opacity="0.45" stroke-width="1.2" />
    <line x1="${minX}" y1="${centerY - 4}" x2="${minX}" y2="${centerY + 4}" stroke="#1f2933" stroke-opacity="0.45" stroke-width="1.2" />
    <line x1="${maxX}" y1="${centerY - 4}" x2="${maxX}" y2="${centerY + 4}" stroke="#1f2933" stroke-opacity="0.45" stroke-width="1.2" />`;
      }

      let annoX = ANNOTATION_X;
      body += `
    <text x="${annoX}" y="${centerY + 4}" font-size="12" font-weight="600" fill="#1f2933">${escapeHtml(row.valueText)}</text>`;
      annoX += 62;

      if (row.ratioText !== null) {
        const ratioColor = row.ratioGood ? '#137333' : '#c5221f';
        body += `
    <text x="${annoX}" y="${centerY + 4}" font-size="11" fill="${ratioColor}">${escapeHtml(row.ratioText)}</text>`;
      }
      annoX += 84;

      if (row.drift !== null) {
        const style = driftStyle(row.drift);
        const pillWidth = row.drift.length * 6 + 14;
        body += `
    <rect x="${annoX}" y="${centerY - 9}" width="${pillWidth}" height="18" rx="9" fill="${style.bg}" />
    <text x="${annoX + pillWidth / 2}" y="${centerY + 4}" font-size="10.5" fill="${style.fg}" text-anchor="middle">${escapeHtml(row.drift)}</text>`;
      }

      body += `
  </g>\n`;
      yOffset += ROW_HEIGHT;
    }
    yOffset += PROJECT_SPACING;
  }

  const plotBottom = yOffset - PROJECT_SPACING + 6;
  let grid = '';
  for (let i = 0; i * axis.step <= axis.max + axis.step / 2; i += 1) {
    const tick = i * axis.step;
    const x = xFor(tick).toFixed(1);
    grid += `<line x1="${x}" y1="${ticksY + 6}" x2="${x}" y2="${plotBottom}" stroke="#e4e7eb" stroke-width="1" />\n`;
    grid += `<text x="${x}" y="${ticksY}" font-size="10" fill="#7b8794" text-anchor="middle">${escapeHtml(panel.tickFormat(tick))}</text>\n`;
  }

  return { svg: grid + svg + body, yEnd: plotBottom };
}

export function renderBenchmarkSvg(input: BenchReportInput, panels: BenchSvgPanels = 'both'): string {
  const { meta, results } = normalizeBenchReport(input);

  const selected: Panel[] = [];
  if (panels !== 'memory') {
    const time = buildTimePanel(results);
    if (time) selected.push(time);
  }
  if (panels !== 'time') {
    const memory = buildMemoryPanel(results);
    if (memory) selected.push(memory);
  }

  const headerHeight = meta ? 64 : 44;
  let y = headerHeight;
  let body = '';
  for (const panel of selected) {
    const rendered = renderPanel(panel, y);
    body += rendered.svg;
    y = rendered.yEnd + 28;
  }

  const footerY = y - 8;
  const chartHeight = footerY + 26;

  let header = `<text x="16" y="28" font-size="17" font-weight="700" fill="#102a43">TypeScript Compilers Benchmark</text>\n`;
  const metaParts = metaSummaryParts(meta);
  if (metaParts.length > 0) {
    header += `<text x="16" y="46" font-size="11" fill="#7b8794">${escapeHtml(metaParts.join('  ·  '))}</text>\n`;
  }

  const footer = `<text x="16" y="${footerY + 8}" font-size="10.5" fill="#9aa5b1">Local-machine-relative regression benchmark · bars show medians, whiskers show min–max · not a cross-machine performance claim</text>`;

  return `<svg xmlns="http://www.w3.org/2000/svg" width="${CHART_WIDTH}" height="${chartHeight}" viewBox="0 0 ${CHART_WIDTH} ${chartHeight}" font-family="-apple-system, 'Segoe UI', 'Helvetica Neue', Arial, sans-serif">
  <rect width="100%" height="100%" rx="8" fill="#ffffff" stroke="#e4e7eb" />
${header}${body}${footer}
</svg>`;
}

function metaTableHtml(meta: BenchReportMeta | undefined): string {
  if (!meta) return '';
  const rows: Array<[string, string]> = [];
  if (meta.timestamp) rows.push(['Timestamp', meta.timestamp]);
  if (meta.gitCommit) rows.push(['Git', meta.gitBranch ? `${meta.gitBranch} @ ${meta.gitCommit}` : meta.gitCommit]);
  if (meta.cpu) rows.push(['CPU', meta.cores ? `${meta.cpu} (${meta.cores} cores)` : meta.cpu]);
  if (meta.platform) rows.push(['Platform', meta.platform]);
  if (meta.nodeVersion) rows.push(['Node', meta.nodeVersion]);
  if (meta.tscVersion || meta.tsgoVersion) {
    const compilers = [
      meta.tscVersion ? `tsc ${meta.tscVersion} (TS 6 baseline)` : null,
      meta.tsgoVersion ? `tsgo ${meta.tsgoVersion} (TS 7 native)` : null,
    ].filter(Boolean).join(' · ');
    rows.push(['Compilers', compilers]);
  }
  if (meta.iterations !== undefined) {
    rows.push(['Iterations', `${meta.iterations}${meta.warmup !== undefined ? ` (+${meta.warmup} warmup)` : ''}`]);
  }
  if (rows.length === 0) return '';
  const cells = rows
    .map(([key, value]) => `<div class="meta-item"><span class="meta-key">${escapeHtml(key)}</span><span class="meta-value">${escapeHtml(value)}</span></div>`)
    .join('\n      ');
  return `<div class="meta-grid">\n      ${cells}\n    </div>`;
}

function timeTableHtml(results: BenchReportResult[]): string {
  const sections = results.map((r) => {
    const rows = TOOLS_ORDER.filter((tool) => r.stats[tool]).map((tool) => {
      const stats = r.stats[tool]!;
      const drift = tool === 'tsc' ? 'baseline' : (r.drift[tool] ?? '');
      const style = driftStyle(drift);
      const speedup = speedupVsTsc(r, tool);
      const speedCell = speedup === null
        ? '<td class="num">—</td>'
        : `<td class="num" style="color:${speedup >= 1 ? '#137333' : '#c5221f'}">${formatSpeedup(speedup).replace(' vs tsc', '')}</td>`;
      const spread = stats.max - stats.min;
      return `<tr>
          <td>${escapeHtml(toolLabelForResult(tool, r))}</td>
          <td class="num"><strong>${stats.median.toFixed(2)}s</strong></td>
          <td class="num">${stats.min.toFixed(2)}s</td>
          <td class="num">${stats.max.toFixed(2)}s</td>
          <td class="num">${spread.toFixed(2)}s</td>
          <td class="num">${stats.runs}</td>
          ${speedCell}
          <td><span class="pill" style="background:${style.bg};color:${style.fg}">${escapeHtml(drift)}</span></td>
        </tr>`;
    }).join('\n        ');
    return `<h3>${escapeHtml(r.project)}</h3>
      <table>
        <thead><tr><th>Tool</th><th class="num">Median</th><th class="num">Min</th><th class="num">Max</th><th class="num">Spread</th><th class="num">Runs</th><th class="num">vs tsc</th><th>Diagnostic drift</th></tr></thead>
        <tbody>
        ${rows}
        </tbody>
      </table>`;
  });
  return sections.join('\n      ');
}

function memoryTableHtml(results: BenchReportResult[]): string {
  const sections = results
    .filter((r) => Object.values(r.memory ?? {}).some((entry) => entry))
    .map((r) => {
      const rows = TOOLS_ORDER.filter((tool) => r.memory?.[tool]).map((tool) => {
        const memory = r.memory![tool]!;
        const ratio = memoryRatioVsTsc(r, tool);
        const ratioCell = ratio === null
          ? '<td class="num">—</td>'
          : `<td class="num" style="color:${ratio <= 1 ? '#137333' : '#c5221f'}">${formatMemoryRatio(ratio).replace(' of tsc', '')}</td>`;
        return `<tr>
          <td>${escapeHtml(toolLabelForResult(tool, r))}</td>
          <td class="num"><strong>${formatBytes(memory.medianBytes)}</strong></td>
          <td class="num">${formatBytes(memory.minBytes)}</td>
          <td class="num">${formatBytes(memory.maxBytes)}</td>
          <td class="num">${memory.runs}</td>
          ${ratioCell}
          <td>${escapeHtml(memory.source)}</td>
        </tr>`;
      }).join('\n        ');
      return `<h3>${escapeHtml(r.project)}</h3>
      <table>
        <thead><tr><th>Tool</th><th class="num">Median peak memory</th><th class="num">Min</th><th class="num">Max</th><th class="num">Runs</th><th class="num">of tsc</th><th>Source</th></tr></thead>
        <tbody>
        ${rows}
        </tbody>
      </table>`;
    });
  return sections.join('\n      ');
}

export function renderBenchmarkHtml(input: BenchReportInput): string {
  const doc = normalizeBenchReport(input);
  const withMemory = hasMemoryData(doc.results);

  const timeContent = `<div class="chart-wrap">${renderBenchmarkSvg(doc, 'time')}</div>
    <h2>Detailed results</h2>
      ${timeTableHtml(doc.results)}`;

  const body = withMemory
    ? `<div class="tabs">
      <input type="radio" name="bench-tab" id="tab-time" checked>
      <input type="radio" name="bench-tab" id="tab-memory">
      <div class="tab-labels">
        <label for="tab-time">Wall time</label>
        <label for="tab-memory">Peak memory</label>
      </div>
      <div class="tab-panel panel-time">
        ${timeContent}
      </div>
      <div class="tab-panel panel-memory">
        <div class="chart-wrap">${renderBenchmarkSvg(doc, 'memory')}</div>
        <h2>Detailed results</h2>
        ${memoryTableHtml(doc.results)}
        <p class="note">Peak memory of the whole compiler process per run (tsc is measured as its Node process; the tsgo shim execs the native binary in place), measured with <code>/usr/bin/time</code>. On macOS this is <code>phys_footprint</code> — the Activity-Monitor-comparable metric; elsewhere it falls back to maximum RSS (see the Source column).</p>
      </div>
    </div>`
    : timeContent;

  return `<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>TypeScript Compilers Benchmark</title>
  <style>
    :root { color-scheme: light; }
    body { font-family: -apple-system, 'Segoe UI', 'Helvetica Neue', Arial, sans-serif; margin: 0; padding: 24px; background: #f5f7fa; color: #1f2933; }
    .container { max-width: 1020px; margin: 0 auto; background: #fff; padding: 28px 32px; border-radius: 12px; box-shadow: 0 2px 8px rgba(16,42,67,0.08); }
    h1 { margin: 0 0 4px; font-size: 1.5em; color: #102a43; }
    h2 { font-size: 1.1em; margin: 28px 0 8px; color: #102a43; }
    h3 { font-size: 1em; margin: 18px 0 6px; color: #334e68; }
    .subtitle { color: #7b8794; margin: 0 0 18px; font-size: 0.92em; }
    .meta-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr)); gap: 8px 24px; padding: 14px 16px; background: #f5f7fa; border-radius: 8px; margin-bottom: 20px; }
    .meta-item { display: flex; flex-direction: column; }
    .meta-key { font-size: 0.72em; text-transform: uppercase; letter-spacing: 0.06em; color: #7b8794; }
    .meta-value { font-size: 0.92em; }
    .chart-wrap { overflow-x: auto; }
    table { border-collapse: collapse; width: 100%; font-size: 0.9em; }
    th, td { text-align: left; padding: 6px 10px; border-bottom: 1px solid #e4e7eb; }
    th { color: #7b8794; font-weight: 600; font-size: 0.85em; }
    td.num, th.num { text-align: right; font-variant-numeric: tabular-nums; }
    .pill { display: inline-block; padding: 2px 10px; border-radius: 999px; font-size: 0.85em; white-space: nowrap; }
    .note { font-size: 0.85em; color: #7b8794; }
    .disclaimer { font-size: 0.85em; color: #7b8794; margin-top: 24px; padding-top: 12px; border-top: 1px solid #e4e7eb; }
    .tabs > input { display: none; }
    .tab-labels { display: flex; gap: 4px; border-bottom: 2px solid #e4e7eb; margin-bottom: 16px; }
    .tab-labels label { padding: 8px 18px; cursor: pointer; font-weight: 600; font-size: 0.95em; color: #7b8794; border-radius: 6px 6px 0 0; margin-bottom: -2px; border-bottom: 2px solid transparent; }
    .tab-labels label:hover { color: #334e68; background: #f5f7fa; }
    .tab-panel { display: none; }
    #tab-time:checked ~ .tab-labels label[for="tab-time"],
    #tab-memory:checked ~ .tab-labels label[for="tab-memory"] { color: #102a43; border-bottom-color: #3178c6; }
    #tab-time:checked ~ .panel-time { display: block; }
    #tab-memory:checked ~ .panel-memory { display: block; }
  </style>
</head>
<body>
  <div class="container">
    <h1>TypeScript Compilers Benchmark</h1>
    <p class="subtitle">Wall-clock and peak-memory comparison of no-emit project checking. Bars show medians; whiskers show the min–max run spread.</p>
    ${metaTableHtml(doc.meta)}
    ${body}
    <div class="disclaimer">
      <strong>Disclaimer:</strong> This is a local-machine-relative regression benchmark. Results are highly dependent on the hardware it was run on. These are not cross-machine or marketing performance claims. Non-exact diagnostic drift means the tool's diagnostics differ from the baseline tsc output; speed is only meaningful alongside a correct diagnostic surface.
    </div>
  </div>
</body>
</html>`;
}
