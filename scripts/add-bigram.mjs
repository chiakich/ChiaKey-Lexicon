#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const BIGRAM_PATH = path.join(ROOT, "sources/chiaki-modern-overlay/bigrams.tsv");
const DEFAULT_PROBABILITY = -0.35;

const USAGE = `Usage:
  node scripts/add-bigram.mjs <previous> <current> <current-keyboard-zhuyin> [probability] [--dry-run] [--force]

Examples:
  node scripts/add-bigram.mjs 天意 難測 "s06hk4" --dry-run
  node scripts/add-bigram.mjs 天意 難測 "s06hk4" -0.5

Notes:
  A bigram fixes the transition previous -> current. The qstring is derived from current.
`;

function main() {
  const options = parseArgs(process.argv.slice(2));
  if (!options.previous || !options.current || !options.keyboard) {
    console.error(USAGE);
    process.exit(1);
  }

  const reading = currentReading(options.current, options.keyboard);
  const probability = options.probability ?? DEFAULT_PROBABILITY;
  const line = `${reading.qstring}\t${options.previous}\t${options.current}\t${formatProbability(probability)}`;
  const existing = loadRows(BIGRAM_PATH).find(
    (row) =>
      row.qstring === reading.qstring &&
      row.previous === options.previous &&
      row.current === options.current,
  );

  console.log(`current bpmf: ${reading.bpmf}`);
  console.log(`current qstring: ${reading.qstring}`);
  console.log(`probability: ${formatProbability(probability)}`);

  if (existing && !options.force) {
    console.error(`\nAlready exists in ${relative(BIGRAM_PATH)}: ${existing.line}`);
    console.error("Use --force to append anyway.");
    process.exit(1);
  }

  if (options.dryRun) {
    console.log(`\nDry run row:\n${line}`);
    return;
  }

  fs.appendFileSync(BIGRAM_PATH, `${line}\n`, "utf8");
  console.log(`\nAppended to ${relative(BIGRAM_PATH)}:\n${line}`);
}

function parseArgs(argv) {
  const positional = [];
  const options = {
    dryRun: false,
    force: false,
    probability: null,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--dry-run") {
      options.dryRun = true;
    } else if (arg === "--force") {
      options.force = true;
    } else if (arg === "--probability") {
      options.probability = parseProbability(requiredValue(argv, ++index, "--probability"));
    } else if (arg.startsWith("--")) {
      throw new Error(`unknown option: ${arg}`);
    } else {
      positional.push(arg);
    }
  }

  options.previous = positional[0];
  options.current = positional[1];
  options.keyboard = positional[2];
  if (positional[3] !== undefined) {
    options.probability = parseProbability(positional[3]);
  }
  if (positional.length > 4) {
    throw new Error(`unexpected argument: ${positional.slice(4).join(" ")}`);
  }
  return options;
}

function requiredValue(argv, index, option) {
  const value = argv[index];
  if (!value || value.startsWith("--")) {
    throw new Error(`${option} requires a value`);
  }
  return value;
}

function parseProbability(value) {
  const probability = Number(value);
  if (!Number.isFinite(probability)) {
    throw new Error(`invalid probability: ${value}`);
  }
  return probability;
}

function currentReading(current, keyboard) {
  const output = execFileSync(process.execPath, [
    "scripts/add-explicit.mjs",
    keyboard,
    current,
    "--dry-run",
  ], {
    cwd: ROOT,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });
  return {
    bpmf: matchLine(output, /^bpmf:\s*(.+)$/m),
    qstring: matchLine(output, /^qstring:\s*(.+)$/m),
  };
}

function loadRows(filePath) {
  if (!fs.existsSync(filePath)) return [];
  return fs
    .readFileSync(filePath, "utf8")
    .split(/\r?\n/)
    .filter((line) => line && !line.startsWith("#"))
    .map((line) => {
      const [qstring, previous, current, probability] = line.split("\t");
      return { qstring, previous, current, probability, line };
    });
}

function matchLine(output, regex) {
  const match = output.match(regex);
  return match ? match[1].trim() : "";
}

function formatProbability(probability) {
  return String(Math.round(probability * 1_000_000) / 1_000_000);
}

function relative(filePath) {
  return path.relative(ROOT, filePath);
}

main();
