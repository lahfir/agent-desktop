#!/usr/bin/env node
import { readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { execFileSync } from "node:child_process";

const __dirname = dirname(fileURLToPath(import.meta.url));
const HISTORY_PATH = join(__dirname, "history.json");
const CONVEX_URL = "https://wry-manatee-359.convex.cloud";

function gh(args) {
  return execFileSync("gh", args, { encoding: "utf8" }).trim();
}

async function fetchClawHubDownloads(slug) {
  const res = await fetch(`${CONVEX_URL}/api/query`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "Convex-Client": "npm-1.20.0",
    },
    body: JSON.stringify({
      path: "skills:getBySlug",
      format: "convex_encoded_json",
      args: [{ slug }],
    }),
  });
  if (!res.ok) throw new Error(`ClawHub HTTP ${res.status}: ${await res.text()}`);
  const body = await res.json();
  if (body.status !== "success") {
    throw new Error(`ClawHub error: ${body.errorMessage ?? "unknown"}`);
  }
  return Math.round(body.value.skill.stats.downloads ?? 0);
}

function upsertPoint(series, iso, value) {
  const last = series[series.length - 1];
  if (last && last[0] === iso) {
    last[1] = value;
    return series;
  }
  return [...series, [iso, value]];
}

async function main() {
  const today = new Date().toISOString().slice(0, 10);
  const raw = await readFile(HISTORY_PATH, "utf8");
  const history = JSON.parse(raw);

  console.log(`[update] repo=${history.repo} slug=${history.slug} today=${today}`);

  const ghStars = parseInt(
    gh(["api", `repos/${history.repo}`, "--jq", ".stargazers_count"]),
    10,
  );
  console.log(`[update] github stars=${ghStars}`);

  const chDownloads = await fetchClawHubDownloads(history.slug);
  console.log(`[update] clawhub downloads=${chDownloads}`);

  history.github_stars = upsertPoint(history.github_stars, today, ghStars);
  history.clawhub_downloads = upsertPoint(
    history.clawhub_downloads,
    today,
    chDownloads,
  );

  await writeFile(HISTORY_PATH, JSON.stringify(history, null, 2) + "\n");
  console.log(`[update] wrote ${HISTORY_PATH}`);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
