#!/usr/bin/env node
import { readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const __dirname = dirname(fileURLToPath(import.meta.url));
const HISTORY_PATH = join(__dirname, "history.json");
const OUT_LIGHT = join(__dirname, "chart-light.svg");
const OUT_DARK = join(__dirname, "chart-dark.svg");

const W = 1200;
const H = 420;
const PAD = { top: 76, right: 72, bottom: 56, left: 72 };
const PLOT_W = W - PAD.left - PAD.right;
const PLOT_H = H - PAD.top - PAD.bottom;

const THEMES = {
  light: {
    bg: "#ffffff",
    surface: "#f6f8fa",
    grid: "#d0d7de",
    gridSoft: "#d0d7de",
    axis: "#656d76",
    text: "#1f2328",
    textSoft: "#656d76",
    titleAccent: "#0969da",
    line1: "#0969da",
    line1End: "#54aeff",
    line2: "#bc4c00",
    line2End: "#fb8500",
    glowOpacity: 0.18,
    fillOpacityTop: 0.18,
    fillOpacityBottom: 0.0,
    cardBorder: "#d0d7de",
  },
  dark: {
    bg: "#0d1117",
    surface: "#161b22",
    grid: "#30363d",
    gridSoft: "#21262d",
    axis: "#8b949e",
    text: "#e6edf3",
    textSoft: "#8b949e",
    titleAccent: "#58a6ff",
    line1: "#58a6ff",
    line1End: "#a5d6ff",
    line2: "#ff7b35",
    line2End: "#ffa657",
    glowOpacity: 0.32,
    fillOpacityTop: 0.28,
    fillOpacityBottom: 0.0,
    cardBorder: "#30363d",
  },
};

function fmtNumber(n) {
  if (n >= 1e6) return (n / 1e6).toFixed(n >= 1e7 ? 0 : 1) + "M";
  if (n >= 1e4) return (n / 1e3).toFixed(0) + "k";
  if (n >= 1e3) return (n / 1e3).toFixed(1) + "k";
  return String(Math.round(n));
}

function fmtDateShort(iso) {
  const d = new Date(iso + "T00:00:00Z");
  return d.toLocaleDateString("en-US", {
    month: "short",
    day: "numeric",
    timeZone: "UTC",
  });
}

function niceMax(v) {
  if (v <= 0) return 10;
  const p = Math.pow(10, Math.floor(Math.log10(v)));
  const f = v / p;
  let nice;
  if (f <= 1) nice = 1;
  else if (f <= 2) nice = 2;
  else if (f <= 5) nice = 5;
  else nice = 10;
  return nice * p;
}

function dateToX(iso, minDay, maxDay) {
  const d = new Date(iso + "T00:00:00Z").getTime() / 86400000;
  const t = (d - minDay) / Math.max(1, maxDay - minDay);
  return PAD.left + t * PLOT_W;
}

function valToY(v, maxV) {
  const t = v / Math.max(1, maxV);
  return PAD.top + PLOT_H - t * PLOT_H;
}

function catmullRomPath(points) {
  if (points.length === 0) return "";
  if (points.length === 1) {
    const [x, y] = points[0];
    return `M ${x.toFixed(2)} ${y.toFixed(2)}`;
  }
  let d = `M ${points[0][0].toFixed(2)} ${points[0][1].toFixed(2)}`;
  for (let i = 0; i < points.length - 1; i++) {
    const p0 = points[i - 1] || points[i];
    const p1 = points[i];
    const p2 = points[i + 1];
    const p3 = points[i + 2] || p2;
    const cp1x = p1[0] + (p2[0] - p0[0]) / 6;
    const cp1y = p1[1] + (p2[1] - p0[1]) / 6;
    const cp2x = p2[0] - (p3[0] - p1[0]) / 6;
    const cp2y = p2[1] - (p3[1] - p1[1]) / 6;
    d += ` C ${cp1x.toFixed(2)} ${cp1y.toFixed(2)}, ${cp2x.toFixed(2)} ${cp2y.toFixed(2)}, ${p2[0].toFixed(2)} ${p2[1].toFixed(2)}`;
  }
  return d;
}

function pathLength(points) {
  let len = 0;
  for (let i = 1; i < points.length; i++) {
    const dx = points[i][0] - points[i - 1][0];
    const dy = points[i][1] - points[i - 1][1];
    len += Math.sqrt(dx * dx + dy * dy);
  }
  return Math.max(len, 1);
}

function gridTicks(maxV, count = 4) {
  const step = maxV / count;
  return Array.from({ length: count + 1 }, (_, i) => i * step);
}

function dateTicks(minDay, maxDay, count = 5) {
  const span = maxDay - minDay;
  const step = span / count;
  return Array.from({ length: count + 1 }, (_, i) => {
    const day = Math.round(minDay + i * step);
    const iso = new Date(day * 86400000).toISOString().slice(0, 10);
    return iso;
  });
}

function renderChart(theme, history, meta) {
  const t = THEMES[theme];
  const series1 = history.github_stars; // [[iso, n], ...]
  const series2 = history.clawhub_downloads;

  const allDates = [...series1, ...series2].map(([iso]) => iso);
  const minIso = allDates.reduce((a, b) => (a < b ? a : b));
  const maxIso = allDates.reduce((a, b) => (a > b ? a : b));
  const minDay = new Date(minIso + "T00:00:00Z").getTime() / 86400000;
  const todayDay = new Date(maxIso + "T00:00:00Z").getTime() / 86400000;
  const maxDay = todayDay; // x-axis ends today

  const max1 = niceMax(Math.max(...series1.map(([, v]) => v), 1));
  const max2 = niceMax(Math.max(...series2.map(([, v]) => v), 1));

  const pts1 = series1.map(([iso, v]) => [
    dateToX(iso, minDay, maxDay),
    valToY(v, max1),
  ]);
  const pts2 = series2.map(([iso, v]) => [
    dateToX(iso, minDay, maxDay),
    valToY(v, max2),
  ]);

  const series2HasHistory = pts2.length >= 2;

  const line1Path = catmullRomPath(pts1);
  const line2Path = series2HasHistory ? catmullRomPath(pts2) : "";

  const area1Path =
    pts1.length >= 2
      ? `${line1Path} L ${pts1[pts1.length - 1][0].toFixed(2)} ${(PAD.top + PLOT_H).toFixed(2)} L ${pts1[0][0].toFixed(2)} ${(PAD.top + PLOT_H).toFixed(2)} Z`
      : "";
  const area2Path = series2HasHistory
    ? `${line2Path} L ${pts2[pts2.length - 1][0].toFixed(2)} ${(PAD.top + PLOT_H).toFixed(2)} L ${pts2[0][0].toFixed(2)} ${(PAD.top + PLOT_H).toFixed(2)} Z`
    : "";

  const yTicks1 = gridTicks(max1);
  const yTicks2 = gridTicks(max2);
  const xTicks = dateTicks(minDay, maxDay);

  const last1 = series1[series1.length - 1]?.[1] ?? 0;
  const last2 = series2[series2.length - 1]?.[1] ?? 0;

  const titleY = 30;
  const subtitleY = 50;

  const lastPt1 = pts1[pts1.length - 1];
  const lastPt2 = pts2[pts2.length - 1];

  const FONT =
    "-apple-system, BlinkMacSystemFont, 'Segoe UI', Helvetica, Arial, sans-serif";

  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${W} ${H}" width="${W}" height="${H}" preserveAspectRatio="xMidYMid meet" role="img" aria-labelledby="title desc">
  <title id="title">${meta.repo} — GitHub stars and ClawHub downloads over time</title>
  <desc id="desc">Time-series chart showing GitHub stars and ClawHub downloads for ${meta.repo}.</desc>
  <defs>
    <linearGradient id="fill1" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0%" stop-color="${t.line1}" stop-opacity="${t.fillOpacityTop}"/>
      <stop offset="100%" stop-color="${t.line1}" stop-opacity="${t.fillOpacityBottom}"/>
    </linearGradient>
    <linearGradient id="fill2" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0%" stop-color="${t.line2}" stop-opacity="${t.fillOpacityTop}"/>
      <stop offset="100%" stop-color="${t.line2}" stop-opacity="${t.fillOpacityBottom}"/>
    </linearGradient>
    <linearGradient id="stroke1" x1="0" y1="0" x2="1" y2="0">
      <stop offset="0%" stop-color="${t.line1}"/>
      <stop offset="100%" stop-color="${t.line1End}"/>
    </linearGradient>
    <linearGradient id="stroke2" x1="0" y1="0" x2="1" y2="0">
      <stop offset="0%" stop-color="${t.line2}"/>
      <stop offset="100%" stop-color="${t.line2End}"/>
    </linearGradient>
    <filter id="glow1" x="-10%" y="-50%" width="120%" height="200%">
      <feGaussianBlur in="SourceGraphic" stdDeviation="3" result="blur"/>
      <feColorMatrix in="blur" type="matrix" values="0 0 0 0 ${hex(t.line1, "r")}  0 0 0 0 ${hex(t.line1, "g")}  0 0 0 0 ${hex(t.line1, "b")}  0 0 0 ${t.glowOpacity} 0" result="coloredGlow"/>
      <feMerge>
        <feMergeNode in="coloredGlow"/>
        <feMergeNode in="SourceGraphic"/>
      </feMerge>
    </filter>
    <filter id="glow2" x="-10%" y="-50%" width="120%" height="200%">
      <feGaussianBlur in="SourceGraphic" stdDeviation="3" result="blur"/>
      <feColorMatrix in="blur" type="matrix" values="0 0 0 0 ${hex(t.line2, "r")}  0 0 0 0 ${hex(t.line2, "g")}  0 0 0 0 ${hex(t.line2, "b")}  0 0 0 ${t.glowOpacity} 0" result="coloredGlow"/>
      <feMerge>
        <feMergeNode in="coloredGlow"/>
        <feMergeNode in="SourceGraphic"/>
      </feMerge>
    </filter>
    <clipPath id="plot-clip">
      <rect x="${PAD.left}" y="${PAD.top - 4}" width="${PLOT_W}" height="${PLOT_H + 8}"/>
    </clipPath>
  </defs>

  <rect width="100%" height="100%" fill="${t.bg}"/>
  <rect x="0.5" y="0.5" width="${W - 1}" height="${H - 1}" fill="none" stroke="${t.cardBorder}" stroke-width="1" rx="8" ry="8"/>

  <text x="${PAD.left}" y="${titleY}" fill="${t.text}" font-family="${FONT}" font-size="18" font-weight="600">
    ${meta.repo}
  </text>
  <text x="${PAD.left}" y="${subtitleY}" fill="${t.textSoft}" font-family="${FONT}" font-size="12" font-weight="400">
    ${meta.subtitle}
  </text>

  <g transform="translate(${W - PAD.right}, ${titleY})" text-anchor="end" font-family="${FONT}" font-size="14" font-weight="600" style="font-variant-numeric: tabular-nums;">
    <text fill="${t.line1}">★ ${fmtNumber(last1)} <tspan fill="${t.textSoft}" font-weight="400" font-size="11">GitHub</tspan></text>
    <text y="20" fill="${t.line2}">⬇ ${fmtNumber(last2)} <tspan fill="${t.textSoft}" font-weight="400" font-size="11">ClawHub</tspan></text>
  </g>

  <g font-family="${FONT}" font-size="10" fill="${t.textSoft}" style="font-variant-numeric: tabular-nums;">
    ${yTicks1
      .map((v, i) => {
        const y = valToY(v, max1);
        const gridLine =
          i === 0
            ? ""
            : `<line x1="${PAD.left}" x2="${W - PAD.right}" y1="${y.toFixed(2)}" y2="${y.toFixed(2)}" stroke="${t.gridSoft}" stroke-width="1" stroke-dasharray="2 4" opacity="0.6"/>`;
        return `${gridLine}<text x="${PAD.left - 8}" y="${(y + 3).toFixed(2)}" text-anchor="end" fill="${t.line1}" opacity="0.85">${fmtNumber(v)}</text>`;
      })
      .join("\n    ")}
  </g>

  <g font-family="${FONT}" font-size="10" fill="${t.line2}" opacity="0.85" style="font-variant-numeric: tabular-nums;">
    ${yTicks2
      .map((v) => {
        const y = valToY(v, max2);
        return `<text x="${W - PAD.right + 8}" y="${(y + 3).toFixed(2)}" text-anchor="start">${fmtNumber(v)}</text>`;
      })
      .join("\n    ")}
  </g>

  <g font-family="${FONT}" font-size="10" fill="${t.textSoft}" style="font-variant-numeric: tabular-nums;">
    ${xTicks
      .map((iso) => {
        const x = dateToX(iso, minDay, maxDay);
        return `<text x="${x.toFixed(2)}" y="${H - PAD.bottom + 18}" text-anchor="middle">${fmtDateShort(iso)}</text>`;
      })
      .join("\n    ")}
  </g>

  <line x1="${PAD.left}" x2="${W - PAD.right}" y1="${PAD.top + PLOT_H}" y2="${PAD.top + PLOT_H}" stroke="${t.grid}" stroke-width="1" opacity="0.8"/>

  <g clip-path="url(#plot-clip)">
    ${area1Path ? `<path d="${area1Path}" fill="url(#fill1)" stroke="none"/>` : ""}
    ${area2Path ? `<path d="${area2Path}" fill="url(#fill2)" stroke="none"/>` : ""}

    ${
      pts1.length >= 2
        ? `<path d="${line1Path}" fill="none" stroke="url(#stroke1)" stroke-width="2.25" stroke-linecap="round" stroke-linejoin="round" filter="url(#glow1)" pathLength="100" stroke-dasharray="100" stroke-dashoffset="100">
      <animate attributeName="stroke-dashoffset" from="100" to="0" dur="1.4s" begin="0s" fill="freeze" calcMode="spline" keySplines="0.4 0 0.2 1" keyTimes="0;1" values="100;0"/>
    </path>`
        : ""
    }

    ${
      series2HasHistory
        ? `<path d="${line2Path}" fill="none" stroke="url(#stroke2)" stroke-width="2.25" stroke-linecap="round" stroke-linejoin="round" filter="url(#glow2)" pathLength="100" stroke-dasharray="100" stroke-dashoffset="100">
      <animate attributeName="stroke-dashoffset" from="100" to="0" dur="1.4s" begin="0.15s" fill="freeze" calcMode="spline" keySplines="0.4 0 0.2 1" keyTimes="0;1" values="100;0"/>
    </path>`
        : lastPt2
          ? `<line x1="${PAD.left}" x2="${(W - PAD.right).toFixed(2)}" y1="${lastPt2[1].toFixed(2)}" y2="${lastPt2[1].toFixed(2)}" stroke="${t.line2}" stroke-width="1.5" stroke-dasharray="5 5" opacity="0">
      <animate attributeName="opacity" from="0" to="0.7" dur="0.6s" begin="0.4s" fill="freeze"/>
    </line>`
          : ""
    }

    ${
      lastPt1
        ? `<g opacity="0">
      <circle cx="${lastPt1[0].toFixed(2)}" cy="${lastPt1[1].toFixed(2)}" r="7" fill="${t.line1}" opacity="0.22"/>
      <circle cx="${lastPt1[0].toFixed(2)}" cy="${lastPt1[1].toFixed(2)}" r="4" fill="${t.bg}" stroke="${t.line1}" stroke-width="2"/>
      <animate attributeName="opacity" from="0" to="1" begin="1.4s" dur="0.4s" fill="freeze"/>
    </g>`
        : ""
    }
    ${
      lastPt2
        ? `<g opacity="0">
      <circle cx="${lastPt2[0].toFixed(2)}" cy="${lastPt2[1].toFixed(2)}" r="7" fill="${t.line2}" opacity="0.22"/>
      <circle cx="${lastPt2[0].toFixed(2)}" cy="${lastPt2[1].toFixed(2)}" r="4" fill="${t.bg}" stroke="${t.line2}" stroke-width="2"/>
      <animate attributeName="opacity" from="0" to="1" begin="${series2HasHistory ? "1.55s" : "1.0s"}" dur="0.4s" fill="freeze"/>
    </g>`
        : ""
    }
  </g>

  ${
    !series2HasHistory && lastPt2
      ? `<text x="${(lastPt2[0] - 12).toFixed(2)}" y="${(lastPt2[1] - 10).toFixed(2)}" text-anchor="end" font-family="${FONT}" font-size="11" font-weight="600" fill="${t.line2}" opacity="0" style="font-variant-numeric: tabular-nums;">
    ${fmtNumber(last2)} downloads
    <animate attributeName="opacity" from="0" to="1" begin="1.2s" dur="0.4s" fill="freeze"/>
  </text>
  <text x="${(lastPt2[0] - 12).toFixed(2)}" y="${(lastPt2[1] + 4).toFixed(2)}" text-anchor="end" font-family="${FONT}" font-size="9" fill="${t.textSoft}" opacity="0">
    daily history starts now
    <animate attributeName="opacity" from="0" to="0.85" begin="1.4s" dur="0.4s" fill="freeze"/>
  </text>`
      : ""
  }

  <text x="${W - PAD.right}" y="${H - 12}" text-anchor="end" font-family="${FONT}" font-size="10" fill="${t.textSoft}" opacity="0.7">
    Updated ${meta.updatedAt}
  </text>
</svg>`;
}

function hex(color, channel) {
  const m = color.match(/^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i);
  if (!m) return "0";
  const r = parseInt(m[1], 16) / 255;
  const g = parseInt(m[2], 16) / 255;
  const b = parseInt(m[3], 16) / 255;
  return ({ r, g, b })[channel].toFixed(3);
}

async function main() {
  const raw = await readFile(HISTORY_PATH, "utf8");
  const history = JSON.parse(raw);
  const meta = {
    repo: history.repo,
    subtitle: history.subtitle ?? "GitHub stars · ClawHub downloads",
    updatedAt: new Date().toISOString().slice(0, 10),
  };

  const lightSvg = renderChart("light", history, meta);
  const darkSvg = renderChart("dark", history, meta);

  await writeFile(OUT_LIGHT, lightSvg);
  await writeFile(OUT_DARK, darkSvg);

  console.log(`Wrote ${OUT_LIGHT}`);
  console.log(`Wrote ${OUT_DARK}`);
  console.log(
    `GitHub stars: ${history.github_stars[history.github_stars.length - 1]?.[1] ?? 0}`,
  );
  console.log(
    `ClawHub downloads: ${history.clawhub_downloads[history.clawhub_downloads.length - 1]?.[1] ?? 0}`,
  );
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
