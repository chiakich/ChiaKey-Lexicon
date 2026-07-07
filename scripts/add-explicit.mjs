#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import readline from "node:readline";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const EXPLICIT_PATH = path.join(ROOT, "sources/chiaki-modern-overlay/explicit.tsv");
const NORMALIZED_PATH =
  process.env.NORMALIZED_PATH ?? path.join(ROOT, "normalized/smart-mandarin.tsv");
const DEFAULT_TAGS = "chiaki-modern-overlay,manual";
const SPLIT_MARGIN = 0.01;
const FALLBACK_WEIGHT = -2.3;

const KEYBOARD_TO_BPMF = new Map([
  ["1", "ㄅ"],
  ["q", "ㄆ"],
  ["a", "ㄇ"],
  ["z", "ㄈ"],
  ["2", "ㄉ"],
  ["w", "ㄊ"],
  ["s", "ㄋ"],
  ["x", "ㄌ"],
  ["e", "ㄍ"],
  ["d", "ㄎ"],
  ["c", "ㄏ"],
  ["r", "ㄐ"],
  ["f", "ㄑ"],
  ["v", "ㄒ"],
  ["5", "ㄓ"],
  ["t", "ㄔ"],
  ["g", "ㄕ"],
  ["b", "ㄖ"],
  ["y", "ㄗ"],
  ["h", "ㄘ"],
  ["n", "ㄙ"],
  ["u", "ㄧ"],
  ["j", "ㄨ"],
  ["m", "ㄩ"],
  ["8", "ㄚ"],
  ["i", "ㄛ"],
  ["k", "ㄜ"],
  [",", "ㄝ"],
  ["9", "ㄞ"],
  ["o", "ㄟ"],
  ["l", "ㄠ"],
  [".", "ㄡ"],
  ["0", "ㄢ"],
  ["p", "ㄣ"],
  [";", "ㄤ"],
  ["/", "ㄥ"],
  ["-", "ㄦ"],
  ["6", "ˊ"],
  ["3", "ˇ"],
  ["4", "ˋ"],
  ["7", "˙"],
]);

const COMPONENTS = [
  ["ㄅ", 0x0001],
  ["ㄆ", 0x0002],
  ["ㄇ", 0x0003],
  ["ㄈ", 0x0004],
  ["ㄉ", 0x0005],
  ["ㄊ", 0x0006],
  ["ㄋ", 0x0007],
  ["ㄌ", 0x0008],
  ["ㄍ", 0x0009],
  ["ㄎ", 0x000a],
  ["ㄏ", 0x000b],
  ["ㄐ", 0x000c],
  ["ㄑ", 0x000d],
  ["ㄒ", 0x000e],
  ["ㄓ", 0x000f],
  ["ㄔ", 0x0010],
  ["ㄕ", 0x0011],
  ["ㄖ", 0x0012],
  ["ㄗ", 0x0013],
  ["ㄘ", 0x0014],
  ["ㄙ", 0x0015],
  ["ㄧ", 0x0020],
  ["ㄨ", 0x0040],
  ["ㄩ", 0x0060],
  ["ㄚ", 0x0080],
  ["ㄛ", 0x0100],
  ["ㄜ", 0x0180],
  ["ㄝ", 0x0200],
  ["ㄞ", 0x0280],
  ["ㄟ", 0x0300],
  ["ㄠ", 0x0380],
  ["ㄡ", 0x0400],
  ["ㄢ", 0x0480],
  ["ㄣ", 0x0500],
  ["ㄤ", 0x0580],
  ["ㄥ", 0x0600],
  ["ㄦ", 0x0680],
  ["ˊ", 0x0800],
  ["ˇ", 0x1000],
  ["ˋ", 0x1800],
  ["˙", 0x2000],
];
const COMPONENT_VALUES = new Map(COMPONENTS);
const TONE_KEYS = new Set(["6", "3", "4", "7"]);

const USAGE = `Usage:
  node scripts/add-explicit.mjs <keyboard-zhuyin> <phrase> [weight] [--tag TAG] [--tags TAGS] [--dry-run]

Examples:
  node scripts/add-explicit.mjs "su3 cl3" 你好 --dry-run
  node scripts/add-explicit.mjs "ek7" 個 -2.9 --tag neutral-tone

Notes:
  Use standard Taiwan Zhuyin keyboard keys: su3 = ㄋㄧˇ, cl3 = ㄏㄠˇ.
  Separate first-tone syllables with spaces because they do not have a tone key.
`;

async function main() {
  const options = parseArgs(process.argv.slice(2));
  if (!options.keyboard || !options.phrase) {
    console.error(USAGE);
    process.exit(1);
  }

  const bpmfSyllables = keyboardToBpmfSyllables(options.keyboard);
  const qstring = qstringForBpmfSyllables(bpmfSyllables);
  const lexicon = fs.existsSync(NORMALIZED_PATH) ? await loadLexicon(NORMALIZED_PATH) : null;
  const explicitRows = await loadExplicitRows(EXPLICIT_PATH);
  const existingExplicit = explicitRows.find(
    (row) => row.qstring === qstring && row.phrase === options.phrase,
  );
  const suggested = options.weight ?? suggestWeight(qstring, options.phrase, lexicon);
  const tags = buildTags(options);
  const line = `${qstring}\t${options.phrase}\t${formatWeight(suggested.weight)}\t${tags}`;

  console.log(`bpmf: ${bpmfSyllables.join(" ")}`);
  console.log(`qstring: ${qstring}`);
  console.log(`weight: ${formatWeight(suggested.weight)} (${suggested.reason})`);
  console.log(`tags: ${tags}`);
  printContext(qstring, options.phrase, lexicon);

  if (existingExplicit && !options.force) {
    console.error(
      `\nAlready exists in ${relative(EXPLICIT_PATH)}: ${existingExplicit.line}`,
    );
    console.error("Use --force to append anyway.");
    process.exit(1);
  }

  if (options.dryRun) {
    console.log(`\nDry run row:\n${line}`);
    return;
  }

  fs.appendFileSync(EXPLICIT_PATH, `${line}\n`, "utf8");
  console.log(`\nAppended to ${relative(EXPLICIT_PATH)}:\n${line}`);
}

function parseArgs(argv) {
  const positional = [];
  const options = {
    tags: DEFAULT_TAGS,
    dryRun: false,
    force: false,
    weight: null,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--dry-run") {
      options.dryRun = true;
    } else if (arg === "--force") {
      options.force = true;
    } else if (arg === "--tag") {
      const tag = requiredValue(argv, ++index, "--tag");
      options.tags = appendTag(options.tags, tag);
    } else if (arg === "--tags") {
      options.tags = requiredValue(argv, ++index, "--tags");
    } else if (arg === "--weight") {
      options.weight = parseWeight(requiredValue(argv, ++index, "--weight"));
    } else if (arg.startsWith("--")) {
      throw new Error(`unknown option: ${arg}`);
    } else {
      positional.push(arg);
    }
  }

  options.keyboard = positional[0];
  options.phrase = positional[1];
  if (positional[2] !== undefined) {
    options.weight = parseWeight(positional[2]);
  }
  if (positional.length > 3) {
    throw new Error(`unexpected argument: ${positional.slice(3).join(" ")}`);
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

function parseWeight(value) {
  const weight = Number(value);
  if (!Number.isFinite(weight)) {
    throw new Error(`invalid weight: ${value}`);
  }
  return weight;
}

function keyboardToBpmfSyllables(input) {
  const trimmed = input.trim().toLowerCase();
  if (!trimmed) {
    throw new Error("keyboard zhuyin is empty");
  }

  const chunks = trimmed.split(/\s+/).filter(Boolean);
  const rawSyllables =
    chunks.length > 1 ? chunks : splitCompactKeyboardSyllables(chunks[0]);
  return rawSyllables.map((syllable) =>
    Array.from(syllable, (key) => {
      const bpmf = KEYBOARD_TO_BPMF.get(key);
      if (!bpmf) {
        throw new Error(`unsupported zhuyin keyboard key: ${key}`);
      }
      return bpmf;
    }).join(""),
  );
}

function splitCompactKeyboardSyllables(input) {
  const syllables = [];
  let current = "";
  for (const key of input) {
    current += key;
    if (TONE_KEYS.has(key)) {
      syllables.push(current);
      current = "";
    }
  }
  if (current) {
    syllables.push(current);
  }
  if (syllables.length <= 1) {
    return syllables;
  }
  return syllables;
}

function qstringForBpmfSyllables(syllables) {
  if (syllables.length === 0) {
    throw new Error("no zhuyin syllables parsed");
  }
  return syllables.map(qstringForBpmfSyllable).join("");
}

function qstringForBpmfSyllable(syllable) {
  const values = Array.from(syllable, (component) => {
    const value = COMPONENT_VALUES.get(component);
    if (!value) {
      throw new Error(`unsupported bopomofo component: ${component}`);
    }
    return value;
  });
  const code = values.reduce((acc, value) => acc | value, 0);
  const order =
    (code & 0x001f) +
    (((code & 0x0060) >> 5) * 22) +
    (((code & 0x0780) >> 7) * 22 * 4) +
    (((code & 0x3800) >> 11) * 22 * 4 * 14);
  return String.fromCodePoint(48 + (order % 79), 48 + Math.floor(order / 79));
}

async function loadLexicon(filePath) {
  const rows = [];
  const byPhrase = new Map();
  const byQstring = new Map();
  const bestByQstring = new Map();

  await forEachLine(filePath, (line) => {
    const [qstring, phrase, weightText, sourceId, tags = ""] = line.split("\t");
    const weight = Number(weightText);
    if (!qstring || !phrase || !Number.isFinite(weight)) return;
    const row = { qstring, phrase, weight, sourceId, tags };
    rows.push(row);
    pushMap(byPhrase, phrase, row);
    pushMap(byQstring, qstring, row);

    const best = bestByQstring.get(qstring);
    if (!best || row.weight > best.weight) {
      bestByQstring.set(qstring, row);
    }
  });

  for (const group of byPhrase.values()) group.sort(sortRows);
  for (const group of byQstring.values()) group.sort(sortRows);
  return { rows, byPhrase, byQstring, bestByQstring };
}

async function loadExplicitRows(filePath) {
  const rows = [];
  await forEachLine(filePath, (line) => {
    const [qstring, phrase] = line.split("\t");
    if (qstring && phrase) rows.push({ qstring, phrase, line });
  });
  return rows;
}

async function forEachLine(filePath, onLine) {
  const stream = fs.createReadStream(filePath, { encoding: "utf8" });
  const rl = readline.createInterface({ input: stream, crlfDelay: Infinity });
  for await (const line of rl) {
    if (!line || line.startsWith("#")) continue;
    onLine(line);
  }
}

function suggestWeight(qstring, phrase, lexicon) {
  if (!lexicon) {
    return {
      weight: FALLBACK_WEIGHT,
      reason: `fallback; ${relative(NORMALIZED_PATH)} not found`,
    };
  }

  const samePhrase = (lexicon.byPhrase.get(phrase) ?? []).filter(
    (row) => row.qstring !== qstring,
  );
  const exact = (lexicon.byPhrase.get(phrase) ?? []).find((row) => row.qstring === qstring);
  if (exact) {
    return {
      weight: exact.weight,
      reason: `matched existing exact normalized row from ${exact.sourceId}`,
    };
  }

  if (samePhrase.length > 0) {
    return {
      weight: samePhrase[0].weight,
      reason: `matched same phrase different qstring ${samePhrase[0].qstring}`,
    };
  }

  const split = bestSplit(qstring, lexicon.bestByQstring);
  if (split) {
    return {
      weight: round6(split.weight + SPLIT_MARGIN),
      reason: `best split ${split.parts.map((row) => row.phrase).join("+")} + ${SPLIT_MARGIN}`,
    };
  }

  const sameQstring = lexicon.byQstring.get(qstring) ?? [];
  if (sameQstring.length > 0) {
    return {
      weight: sameQstring[0].weight,
      reason: `matched strongest same qstring phrase ${sameQstring[0].phrase}`,
    };
  }

  return { weight: FALLBACK_WEIGHT, reason: "fallback; no lexicon context found" };
}

function bestSplit(qstring, bestByQstring) {
  const syllableCount = qstring.length / 2;
  if (!Number.isInteger(syllableCount) || syllableCount < 2) return null;

  let best = null;
  for (let splitSyllable = 1; splitSyllable < syllableCount; splitSyllable += 1) {
    const leftQstring = qstring.slice(0, splitSyllable * 2);
    const rightQstring = qstring.slice(splitSyllable * 2);
    const left = bestByQstring.get(leftQstring);
    const rightSplit = bestSplit(rightQstring, bestByQstring);
    const right = rightSplit ?? bestByQstring.get(rightQstring);
    if (!left || !right) continue;

    const parts = [left, ...(right.parts ?? [right])];
    const weight = left.weight + right.weight;
    if (!best || weight > best.weight) {
      best = { weight, parts };
    }
  }
  return best;
}

function printContext(qstring, phrase, lexicon) {
  if (!lexicon) {
    console.log(`context: ${relative(NORMALIZED_PATH)} not found`);
    return;
  }

  const samePhrase = lexicon.byPhrase.get(phrase) ?? [];
  if (samePhrase.length > 0) {
    console.log("\nsame phrase readings:");
    for (const row of samePhrase.slice(0, 8)) printRow(row, row.qstring === qstring ? "*" : " ");
  }

  const sameQstring = lexicon.byQstring.get(qstring) ?? [];
  if (sameQstring.length > 0) {
    console.log("\nsame qstring ranking:");
    for (const row of sameQstring.slice(0, 8)) printRow(row, row.phrase === phrase ? "*" : " ");
  }

  const split = bestSplit(qstring, lexicon.bestByQstring);
  if (split) {
    console.log(
      `\nbest split: ${split.parts
        .map((row) => `${row.phrase}(${row.qstring}, ${formatWeight(row.weight)})`)
        .join(" + ")} = ${formatWeight(split.weight)}`,
    );
  }
}

function printRow(row, marker) {
  console.log(
    `  ${marker} ${row.qstring}\t${row.phrase}\t${formatWeight(row.weight)}\t${row.sourceId}\t${row.tags}`,
  );
}

function pushMap(map, key, value) {
  if (!map.has(key)) map.set(key, []);
  map.get(key).push(value);
}

function sortRows(left, right) {
  return right.weight - left.weight || left.phrase.localeCompare(right.phrase, "zh-Hant");
}

function buildTags(options) {
  return options.tags
    .split(",")
    .map((tag) => tag.trim())
    .filter(Boolean)
    .join(",");
}

function appendTag(tags, tag) {
  return `${tags},${tag}`
    .split(",")
    .map((part) => part.trim())
    .filter(Boolean)
    .join(",");
}

function formatWeight(weight) {
  return String(round6(weight));
}

function round6(value) {
  return Math.round(value * 1_000_000) / 1_000_000;
}

function relative(filePath) {
  return path.relative(ROOT, filePath);
}

main().catch((error) => {
  console.error(error.message);
  process.exit(1);
});
