#!/usr/bin/env node
import { writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { execFileSync } from "node:child_process";

const __dirname = dirname(fileURLToPath(import.meta.url));
const HISTORY_PATH = join(__dirname, "history.json");

const REPO = process.env.STATS_REPO || "lahfir/agent-desktop";
const SLUG = process.env.CLAWHUB_SLUG || "agent-desktop";
const CONVEX_URL = "https://wry-manatee-359.convex.cloud";

function gh(args) {
  return execFileSync("gh", args, { encoding: "utf8", maxBuffer: 50 * 1024 * 1024 });
}

async function fetchClawHubStats(slug) {
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
  return body.value.skill.stats;
}

function fetchStarHistory(repo) {
  const out = gh([
    "api",
    "-H",
    "Accept: application/vnd.github.star+json",
    "--paginate",
    `repos/${repo}/stargazers`,
    "--jq",
    "[.[] | .starred_at]",
  ]);
  const events = out
    .split("\n")
    .filter(Boolean)
    .flatMap((chunk) => JSON.parse(chunk));
  return events.sort();
}

function bucketCumulative(starredAt) {
  const counts = new Map();
  let cum = 0;
  for (const iso of starredAt) {
    cum += 1;
    counts.set(iso.slice(0, 10), cum);
  }
  return [...counts.entries()].sort();
}

function fillDailyForward(buckets, untilIso) {
  if (buckets.length === 0) return [];
  const out = [];
  const startMs = new Date(buckets[0][0] + "T00:00:00Z").getTime();
  const endMs = new Date(untilIso + "T00:00:00Z").getTime();
  const days = Math.round((endMs - startMs) / 86400000);
  let bi = 0;
  let cur = buckets[0][1];
  for (let i = 0; i <= days; i++) {
    const dayMs = startMs + i * 86400000;
    const iso = new Date(dayMs).toISOString().slice(0, 10);
    while (bi < buckets.length && buckets[bi][0] <= iso) {
      cur = buckets[bi][1];
      bi++;
    }
    out.push([iso, cur]);
  }
  return out;
}

async function main() {
  const today = new Date().toISOString().slice(0, 10);
  console.log(`[seed] repo=${REPO} slug=${SLUG} today=${today}`);

  console.log("[seed] fetching GitHub star history...");
  const starredAt = fetchStarHistory(REPO);
  console.log(`[seed] got ${starredAt.length} star events`);

  const buckets = bucketCumulative(starredAt);
  const ghDaily = fillDailyForward(buckets, today);
  console.log(`[seed] github series: ${ghDaily.length} daily points`);

  console.log("[seed] fetching ClawHub stats...");
  const stats = await fetchClawHubStats(SLUG);
  const downloads = Math.round(stats.downloads ?? 0);
  console.log(`[seed] clawhub downloads=${downloads} stars=${stats.stars ?? 0}`);

  const history = {
    repo: REPO,
    slug: SLUG,
    subtitle: "GitHub stars · ClawHub downloads — updated daily",
    github_stars: ghDaily,
    clawhub_downloads: [[today, downloads]],
    sources: {
      github: `https://github.com/${REPO}`,
      clawhub: `https://clawhub.ai/${REPO.split("/")[0]}/${SLUG}`,
      convex: CONVEX_URL,
    },
    schema_version: 1,
  };

  await writeFile(HISTORY_PATH, JSON.stringify(history, null, 2) + "\n");
  console.log(`[seed] wrote ${HISTORY_PATH}`);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
