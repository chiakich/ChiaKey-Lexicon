#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";

const DEFAULT_SOURCE_ID = "chiaki-auto-hotwords-overlay";
const DEFAULT_GEO = "TW";
const DEFAULT_HL = "zh-TW";
const DEFAULT_RETENTION_DAYS = 30;
const DEFAULT_STATE_WINDOW_DAYS = 90;
const MIN_EMIT_SIGNAL = 3;
const MAX_SEGMENT_DERIVATION_SURFACE_LENGTH = 8;

const WINDOWS = [
  { label: "24h", hours: 24, score: 1 },
  { label: "48h", hours: 48, score: 2 },
  { label: "7d", hours: 168, score: 3 },
];

const QUERY_LIKE_TERMS = [
  "天氣",
  "股價",
  "目標價",
  "盤後",
  "指數",
  "排名",
  "戰績",
  "開獎",
  "匯率",
  "直播",
];

const CORE_CANDIDATE_SUFFIXES = [
  "路況",
  "車禍",
];

const DERIVED_SEGMENT_STOP_CHARS = new Set(["對", "的", "是", "嗎"]);

const PHRASE_REPLACEMENTS = [
  ["世界杯", "世界盃"],
  ["台湾", "台灣"],
  ["新闻", "新聞"],
  ["台风", "颱風"],
];

const CHAR_REPLACEMENTS = new Map([
  ["国", "國"],
  ["际", "際"],
  ["战", "戰"],
  ["绩", "績"],
  ["排", "排"],
  ["湾", "灣"],
  ["彩", "彩"],
  ["券", "券"],
  ["达", "達"],
  ["电", "電"],
  ["风", "風"],
  ["线", "線"],
  ["台", "台"],
  ["龙", "龍"],
  ["韩", "韓"],
  ["盘", "盤"],
  ["后", "後"],
  ["号", "號"],
  ["潜", "潛"],
  ["舰", "艦"],
  ["对", "對"],
  ["马", "馬"],
  ["乌", "烏"],
  ["库", "庫"],
  ["亚", "亞"],
  ["万", "萬"],
  ["废", "廢"],
  ["学", "學"],
  ["师", "師"],
  ["报", "報"],
  ["华", "華"],
  ["县", "縣"],
  ["陈", "陳"],
  ["凤", "鳳"],
  ["积", "積"],
  ["胶", "膠"],
  ["军", "軍"],
  ["张", "張"],
  ["吴", "吳"],
  ["东", "東"],
  ["谚", "諺"],
  ["贤", "賢"],
  ["绮", "綺"],
  ["农", "農"],
  ["贴", "貼"],
  ["习", "習"],
  ["视", "視"],
  ["导", "導"],
  ["体", "體"],
  ["矶", "磯"],
]);

const USAGE = `Usage:
  node scripts/hotwords.mjs collect --output tmp/hotwords-observations/DATE.json [--date YYYY-MM-DD]
  node scripts/hotwords.mjs refresh --observations-dir tmp/hotwords-observations --state sources/chiaki-auto-hotwords-overlay/state.json --output sources/chiaki-auto-hotwords-overlay/phrases.tsv --summary tmp/hotwords-summary.md [--today YYYY-MM-DD]
`;

async function main() {
  const [command, ...argv] = process.argv.slice(2);
  const options = parseArgs(argv);

  if (command === "collect") {
    await collect(options);
  } else if (command === "refresh") {
    await refresh(options);
  } else {
    console.error(USAGE);
    process.exit(command ? 1 : 0);
  }
}

function parseArgs(argv) {
  const options = {};
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (!arg.startsWith("--")) {
      throw new Error(`unexpected argument: ${arg}`);
    }
    const key = arg.slice(2);
    const next = argv[index + 1];
    if (!next || next.startsWith("--")) {
      options[key] = true;
    } else {
      options[key] = next;
      index += 1;
    }
  }
  return options;
}

async function collect(options) {
  const geo = String(options.geo || DEFAULT_GEO);
  const hl = String(options.hl || DEFAULT_HL);
  const observedOn = String(options.date || taipeiDate());
  const collectedAt = new Date().toISOString();
  const output = options.output || path.join("tmp", "hotwords-observations", `${observedOn}.json`);
  const observations = [];
  const fetchedRows = {};

  for (const window of WINDOWS) {
    const rows = await fetchGoogleTrends({ geo, hl, windowHours: window.hours });
    fetchedRows[window.label] = rows.length;
    observations.push(
      ...dedupeByTerm(
        rows
          .map((row) => ({
            term: normalizeTerm(row.term),
            traffic: row.traffic,
            growth_pct: row.growthPct,
            started_at: row.startedAt,
            window_hours: window.hours,
            window_label: window.label,
          }))
          .filter((row) => row.term && isHanOnly(row.term) && !hasAsciiAlnum(row.term)),
      ),
    );
  }

  const payload = {
    schema_version: 2,
    source: "google-trends-trending",
    geo,
    hl,
    windows: WINDOWS.map((window) => ({ label: window.label, hours: window.hours })),
    observed_on: observedOn,
    collected_at: collectedAt,
    fetched_rows_by_window: fetchedRows,
    fetched_rows: Object.values(fetchedRows).reduce((sum, value) => sum + value, 0),
    observations,
  };

  writeJson(output, payload);
  console.log(
    `Collected ${observations.length} normalized hotword observations from ${payload.fetched_rows} Trends rows into ${output}`,
  );
}

async function refresh(options) {
  const observationsDir = requiredOption(options, "observations-dir");
  const statePath = requiredOption(options, "state");
  const outputPath = requiredOption(options, "output");
  const summaryPath = options.summary;
  const normalizedPath = String(options.normalized || "normalized/smart-mandarin.tsv");
  const today = String(options.today || taipeiDate());
  const sourceId = String(options["source-id"] || DEFAULT_SOURCE_ID);

  const observations = loadObservations(observationsDir);
  const state = loadState(statePath);
  const lexicon = loadLexicon(normalizedPath, sourceId);
  const aggregate = mergeState(state, observations, today, lexicon);
  const result = buildOverlayRows(aggregate, lexicon, today, sourceId);
  const outputState = filterStateByTerms(aggregate, result.watchlistTerms);

  writeJson(statePath, buildStatePayload(outputState, today));
  writePhrases(outputPath, result.rows);

  const summary = buildSummary({
    today,
    observations,
    rows: result.rows,
    filtered: result.filtered,
    stateTerms: outputState.size,
    sourceId,
  });
  if (summaryPath) {
    writeText(summaryPath, summary);
  }
  console.log(summary);
}

async function fetchGoogleTrends({ geo, hl, windowHours }) {
  const pageUrl = `https://trends.google.com/trending?geo=${encodeURIComponent(geo)}&hl=${encodeURIComponent(hl)}`;
  const html = await fetchText(pageUrl, {
    headers: { "user-agent": "Mozilla/5.0" },
  });
  const sid = html.match(/"FdrFJe":"([^"]+)"/)?.[1];
  const bl = html.match(/"cfb2h":"([^"]+)"/)?.[1];
  if (!sid || !bl) {
    throw new Error(`Unable to find Google Trends request tokens: sid=${Boolean(sid)} bl=${Boolean(bl)}`);
  }

  const rpcUrl = new URL("https://trends.google.com/_/TrendsUi/data/batchexecute");
  rpcUrl.searchParams.set("rpcids", "i0OFE");
  rpcUrl.searchParams.set("source-path", "/trending");
  rpcUrl.searchParams.set("f.sid", sid);
  rpcUrl.searchParams.set("bl", bl);
  rpcUrl.searchParams.set("hl", hl);
  rpcUrl.searchParams.set("_reqid", "1");
  rpcUrl.searchParams.set("rt", "c");

  const requestPayload = JSON.stringify([
    [["i0OFE", JSON.stringify([null, null, geo, 0, hl, windowHours, 1]), null, "generic"]],
  ]);
  const body = new URLSearchParams({ "f.req": requestPayload });
  const text = await fetchText(rpcUrl.toString(), {
    method: "POST",
    headers: {
      "content-type": "application/x-www-form-urlencoded;charset=UTF-8",
      "x-same-domain": "1",
      referer: pageUrl,
      "user-agent": "Mozilla/5.0",
    },
    body,
  });
  const line = text.split("\n").find((item) => item.startsWith('[["wrb.fr","i0OFE"'));
  if (!line) {
    throw new Error("Google Trends response did not include the i0OFE payload");
  }
  const outer = JSON.parse(line);
  const payload = JSON.parse(outer[0][2]);
  const rows = payload[1] || [];
  return rows
    .map((row) => ({
      term: row[0],
      traffic: numberOrNull(row[6]),
      growthPct: numberOrNull(row[8]),
      startedAt: row[3]?.[0] ? new Date(row[3][0] * 1000).toISOString() : null,
    }))
    .filter((row) => row.term);
}

async function fetchText(url, init) {
  const response = await fetch(url, init);
  if (!response.ok) {
    throw new Error(`HTTP ${response.status} fetching ${url}`);
  }
  return response.text();
}

function loadObservations(root) {
  const files = listJsonFiles(root);
  const observations = [];
  for (const file of files) {
    const payload = JSON.parse(fs.readFileSync(file, "utf8"));
    const observedOn = payload.observed_on || dateOnly(payload.collected_at) || dateOnly(payload.generated_at);
    for (const observation of payload.observations || []) {
      const term = normalizeTerm(observation.term);
      if (!observedOn || !term) {
        continue;
      }
      const windowHours = numberOrNull(observation.window_hours ?? payload.window_hours) || 24;
      observations.push({
        term,
        observed_on: observedOn,
        window_hours: windowHours,
        window_label: observation.window_label || windowLabelForHours(windowHours),
        traffic: numberOrNull(observation.traffic),
        growth_pct: numberOrNull(observation.growth_pct),
      });
    }
  }
  return observations;
}

function loadState(file) {
  if (!fs.existsSync(file)) {
    return new Map();
  }
  const payload = JSON.parse(fs.readFileSync(file, "utf8"));
  const terms = payload.terms || {};
  const state = new Map();
  for (const [term, value] of Object.entries(terms)) {
    const normalized = normalizeTerm(term);
    if (!normalized) {
      continue;
    }
    state.set(normalized, {
      first_seen: value.first_seen,
      last_seen: value.last_seen,
      seen_dates: Array.isArray(value.seen_dates) ? value.seen_dates.filter(isDateOnly) : [],
      seen_windows: normalizeSeenWindows(value.seen_windows, value.seen_dates),
      max_traffic: numberOrNull(value.max_traffic),
      derived_from: Array.isArray(value.derived_from) ? value.derived_from.map(normalizeTerm).filter(Boolean) : [],
    });
  }
  return state;
}

function mergeState(state, observations, today, lexicon) {
  const merged = new Map();
  for (const [term, value] of state.entries()) {
    merged.set(term, {
      first_seen: value.first_seen,
      last_seen: value.last_seen,
      seen_dates: new Set(value.seen_dates || []),
      seen_windows: mapSeenWindowsToSets(value.seen_windows || {}),
      max_traffic: value.max_traffic || 0,
      derived_from: new Set(value.derived_from || []),
    });
  }

  for (const observation of observations) {
    for (const candidate of observationCandidates(observation, lexicon)) {
      mergeObservation(merged, candidate);
    }
  }

  const cutoff = addDays(today, -DEFAULT_STATE_WINDOW_DAYS);
  for (const [term, entry] of merged.entries()) {
    entry.seen_dates = new Set([...entry.seen_dates].filter((date) => date >= cutoff && date <= today));
    entry.seen_windows = pruneSeenWindows(entry.seen_windows, cutoff, today);
    if (entry.seen_dates.size === 0 || daysBetween(entry.last_seen, today) > DEFAULT_RETENTION_DAYS) {
      merged.delete(term);
    }
  }
  return merged;
}

function observationCandidates(observation, lexicon) {
  const term = normalizeTerm(observation.term);
  if (!term || !isHanOnly(term) || hasAsciiAlnum(term)) {
    return [];
  }
  return dedupeObservationCandidates([
    observationWithTerm(observation, term),
    ...deriveCoreCandidates(observation, term, lexicon),
    ...deriveSegmentCandidates(observation, term, lexicon),
  ]);
}

function deriveCoreCandidates(observation, term, lexicon) {
  const tokens = tokenize(term, lexicon);
  if (tokens.join("") !== term || tokens.length < 2) {
    return [];
  }
  const suffix = CORE_CANDIDATE_SUFFIXES.find((item) => tokens.at(-1) === item);
  if (!suffix) {
    return [];
  }
  const core = tokens.slice(0, -1).join("");
  const coreLength = Array.from(core).length;
  if (coreLength < 2 || coreLength > 4 || isQueryLikeTerm(core)) {
    return [];
  }
  return [
    {
      ...observationWithTerm(observation, core),
      derived_from: term,
      derived_suffix: suffix,
    },
  ];
}

function deriveSegmentCandidates(observation, term, lexicon) {
  const termLength = Array.from(term).length;
  if (termLength <= 4 || termLength > MAX_SEGMENT_DERIVATION_SURFACE_LENGTH || isQueryLikeTerm(term)) {
    return [];
  }
  const tokens = tokenize(term, lexicon);
  if (tokens.join("") !== term || tokens.length < 2) {
    return [];
  }

  const candidates = [];
  for (const run of unknownTokenRuns(tokens)) {
    const candidate = run.join("");
    const candidateLength = Array.from(candidate).length;
    if (
      candidateLength >= 2 &&
      candidateLength <= 4 &&
      candidate !== term &&
      !isQueryLikeTerm(candidate) &&
      !hasDerivedSegmentStopChar(candidate)
    ) {
      candidates.push({
        ...observationWithTerm(observation, candidate),
        derived_from: term,
      });
    }
  }
  return candidates;
}

function hasDerivedSegmentStopChar(candidate) {
  return Array.from(candidate).some((character) => DERIVED_SEGMENT_STOP_CHARS.has(character));
}

function unknownTokenRuns(tokens) {
  const runs = [];
  let current = [];
  for (const token of tokens) {
    if (Array.from(token).length === 1) {
      current.push(token);
    } else if (current.length > 0) {
      runs.push(current);
      current = [];
    }
  }
  if (current.length > 0) {
    runs.push(current);
  }
  return runs;
}

function dedupeObservationCandidates(candidates) {
  const byTerm = new Map();
  for (const candidate of candidates) {
    const term = normalizeTerm(candidate.term);
    if (!term || byTerm.has(term)) {
      continue;
    }
    byTerm.set(term, candidate);
  }
  return [...byTerm.values()];
}

function observationWithTerm(observation, term) {
  return {
    ...observation,
    term,
  };
}

function mergeObservation(merged, observation) {
  const term = normalizeTerm(observation.term);
  if (!term || !isHanOnly(term) || hasAsciiAlnum(term)) {
    return;
  }
  const entry =
    merged.get(term) ||
    {
      first_seen: observation.observed_on,
      last_seen: observation.observed_on,
      seen_dates: new Set(),
      seen_windows: {},
      max_traffic: 0,
      derived_from: new Set(),
    };
  entry.seen_dates.add(observation.observed_on);
  const windowLabel = observation.window_label || windowLabelForHours(observation.window_hours || 24);
  entry.seen_windows[windowLabel] ||= new Set();
  entry.seen_windows[windowLabel].add(observation.observed_on);
  entry.first_seen = minDate(entry.first_seen, observation.observed_on);
  entry.last_seen = maxDate(entry.last_seen, observation.observed_on);
  entry.max_traffic = Math.max(entry.max_traffic || 0, observation.traffic || 0);
  if (observation.derived_from) {
    entry.derived_from.add(observation.derived_from);
  }
  merged.set(term, entry);
}

function buildOverlayRows(state, lexicon, today, sourceId) {
  const rows = [];
  const watchlistTerms = new Set();
  const filtered = {
    too_short_or_long: [],
    query_like: [],
    existing_phrase: [],
    typeable_by_top_segments: [],
    non_han: [],
    missing_character_reading: [],
    weak_signal: [],
    expired: [],
  };

  for (const [term, entry] of [...state.entries()].sort(([a], [b]) => a.localeCompare(b))) {
    const length = Array.from(term).length;
    if (!isHanOnly(term)) {
      filtered.non_han.push(term);
      continue;
    }
    if (length < 2 || length > 4) {
      filtered.too_short_or_long.push(term);
      continue;
    }
    if (isQueryLikeTerm(term)) {
      filtered.query_like.push(term);
      continue;
    }
    if (lexicon.byPhrase.has(term)) {
      filtered.existing_phrase.push(term);
      continue;
    }
    const segmentation = typeableByTopSegments(term, lexicon);
    if (segmentation.typeable) {
      filtered.typeable_by_top_segments.push(`${term} (${segmentation.tokens.join(" ")})`);
      continue;
    }
    if (!canInferQstring(term, lexicon)) {
      filtered.missing_character_reading.push(term);
      continue;
    }

    const daysSinceSeen = daysBetween(entry.last_seen, today);
    if (daysSinceSeen > DEFAULT_RETENTION_DAYS) {
      filtered.expired.push(term);
      continue;
    }
    const signal = signalFor(entry, today);
    if (!shouldKeepInState(signal)) {
      filtered.weak_signal.push(formatWeakSignal(term, signal));
      continue;
    }
    watchlistTerms.add(term);
    if (!shouldEmitSignal(signal)) {
      filtered.weak_signal.push(formatWeakSignal(term, signal));
      continue;
    }
    const seenLast14 = countSeenSince(entry.seen_dates, addDays(today, -13));
    const seenLast30 = countSeenSince(entry.seen_dates, addDays(today, -29));
    const weight = weightFor({ daysSinceSeen, signal14: signal.score14, signal30: signal.score30 });
    const tags = [
      sourceId,
      "auto",
      "google-trends",
      `first_seen=${entry.first_seen}`,
      `last_seen=${entry.last_seen}`,
      `seen_days_30=${seenLast30}`,
      `signal_14=${signal.score14}`,
      `signal_30=${signal.score30}`,
      `windows_14=${signal.windows14.join("+")}`,
      `max_traffic=${entry.max_traffic || 0}`,
    ];
    const derivedFrom = sortedDerivedFrom(entry).slice(0, 3);
    if (derivedFrom.length > 0) {
      tags.push(`derived_from=${derivedFrom.join("+")}`);
    }
    rows.push({
      phrase: term,
      weight,
      tags: tags.join(","),
    });
  }
  return { rows, filtered, watchlistTerms };
}

function loadLexicon(file, excludedSourceId) {
  const byPhrase = new Map();
  const byQstring = new Map();

  for (const line of fs.readFileSync(file, "utf8").split(/\r?\n/)) {
    if (!line || line.startsWith("#")) {
      continue;
    }
    const [qstring, phrase, weightText, sourceId] = line.split("\t");
    if (!qstring || !phrase || phrase.includes("_") || sourceId === excludedSourceId) {
      continue;
    }
    const weight = Number(weightText);
    if (!Number.isFinite(weight)) {
      continue;
    }
    const entry = { qstring, phrase, weight, sourceId };
    const previous = byPhrase.get(phrase);
    if (!previous || weight > previous.weight) {
      byPhrase.set(phrase, entry);
    }
    if (!byQstring.has(qstring)) {
      byQstring.set(qstring, []);
    }
    byQstring.get(qstring).push(entry);
  }

  const rankByQstringPhrase = new Map();
  for (const [qstring, entries] of byQstring.entries()) {
    entries.sort((left, right) => right.weight - left.weight || left.phrase.localeCompare(right.phrase));
    entries.forEach((entry, index) => {
      rankByQstringPhrase.set(`${qstring}\0${entry.phrase}`, index + 1);
    });
  }

  return { byPhrase, byQstring, rankByQstringPhrase };
}

function typeableByTopSegments(term, lexicon) {
  const tokens = tokenize(term, lexicon);
  const typeable =
    tokens.length > 1 &&
    tokens.join("") === term &&
    tokens.every((token) => {
      const entry = lexicon.byPhrase.get(token);
      return entry && lexicon.rankByQstringPhrase.get(`${entry.qstring}\0${token}`) === 1;
    });
  return { tokens, typeable };
}

function tokenize(term, lexicon) {
  const chars = Array.from(term);
  const scores = Array(chars.length + 1).fill(Number.NEGATIVE_INFINITY);
  const next = Array(chars.length + 1).fill(null);
  scores[chars.length] = 0;

  for (let index = chars.length - 1; index >= 0; index -= 1) {
    const maxLength = Math.min(7, chars.length - index);
    for (let length = 1; length <= maxLength; length += 1) {
      const candidate = chars.slice(index, index + length).join("");
      const entry = lexicon.byPhrase.get(candidate);
      if (!entry) {
        continue;
      }
      const score = entry.weight + scores[index + length];
      if (score > scores[index] || (score === scores[index] && next[index] && length > next[index].length)) {
        scores[index] = score;
        next[index] = { token: candidate, length };
      }
    }
  }

  const tokens = [];
  let index = 0;
  while (index < chars.length) {
    if (next[index]) {
      tokens.push(next[index].token);
      index += next[index].length;
    } else {
      tokens.push(chars[index]);
      index += 1;
    }
  }
  return tokens;
}

function canInferQstring(term, lexicon) {
  return Array.from(term).every((character) => {
    const entry = lexicon.byPhrase.get(character);
    return entry && Array.from(entry.phrase).length === 1;
  });
}

function isQueryLikeTerm(term) {
  return [...QUERY_LIKE_TERMS, ...CORE_CANDIDATE_SUFFIXES].some((needle) => term.includes(needle));
}

function signalFor(entry, today) {
  const cutoff14 = addDays(today, -13);
  const cutoff30 = addDays(today, -29);
  const windows14 = windowsSeenSince(entry.seen_windows, cutoff14);
  const seenDays14 = countSeenSince(entry.seen_dates, cutoff14);
  return {
    score14: signalScoreSince(entry.seen_windows, cutoff14),
    score30: signalScoreSince(entry.seen_windows, cutoff30),
    windows14,
    seenDays14,
    hasShortWindow14: windows14.includes("24h") || windows14.includes("48h"),
  };
}

function shouldKeepInState(signal) {
  return signal.hasShortWindow14 || signal.windows14.length >= 2 || signal.seenDays14 >= 2;
}

function shouldEmitSignal(signal) {
  return (
    (signal.hasShortWindow14 && signal.windows14.length >= 2 && signal.score14 >= MIN_EMIT_SIGNAL) ||
    (signal.seenDays14 >= 3 && signal.score14 >= MIN_EMIT_SIGNAL)
  );
}

function formatWeakSignal(term, signal) {
  return `${term} (score_14=${signal.score14}, days_14=${signal.seenDays14}, windows=${signal.windows14.join(",") || "none"})`;
}

function signalScoreSince(seenWindows, cutoff) {
  let score = 0;
  for (const window of WINDOWS) {
    for (const date of seenWindows[window.label] || []) {
      if (date >= cutoff) {
        score += window.score;
      }
    }
  }
  return score;
}

function windowsSeenSince(seenWindows, cutoff) {
  return WINDOWS.filter((window) => [...(seenWindows[window.label] || [])].some((date) => date >= cutoff)).map(
    (window) => window.label,
  );
}

function weightFor({ daysSinceSeen, signal14, signal30 }) {
  if (daysSinceSeen > 14) {
    return "-2.6";
  }
  if (signal30 >= 12) {
    return "-1.9";
  }
  if (signal14 >= 6) {
    return "-2.1";
  }
  return "-2.4";
}

function buildStatePayload(state, today) {
  const terms = {};
  for (const [term, entry] of [...state.entries()].sort(([a], [b]) => a.localeCompare(b))) {
    const seenDates = [...entry.seen_dates].sort();
    const seenWindows = serializeSeenWindows(entry.seen_windows);
    const signal = signalFor(entry, today);
    terms[term] = {
      first_seen: entry.first_seen,
      last_seen: entry.last_seen,
      seen_dates: seenDates,
      seen_windows: seenWindows,
      seen_days: seenDates.length,
      signal_14: signal.score14,
      signal_30: signal.score30,
      max_traffic: entry.max_traffic || 0,
    };
    const derivedFrom = sortedDerivedFrom(entry);
    if (derivedFrom.length > 0) {
      terms[term].derived_from = derivedFrom.slice(0, 10);
    }
  }
  return {
    schema_version: 2,
    updated_at: new Date().toISOString(),
    updated_on: today,
    retention_days: DEFAULT_RETENTION_DAYS,
    terms,
  };
}

function sortedDerivedFrom(entry) {
  return [...(entry.derived_from || [])].sort();
}

function filterStateByTerms(state, kept) {
  const filtered = new Map();
  for (const [term, entry] of state.entries()) {
    if (kept.has(term)) {
      filtered.set(term, entry);
    }
  }
  return filtered;
}

function buildSummary({ today, observations, rows, filtered, stateTerms, sourceId }) {
  const lines = [];
  const filteredCounts = Object.entries(filtered)
    .map(([reason, values]) => `${reason}=${values.length}`)
    .join(", ");

  lines.push(`# Auto Hotwords Refresh`);
  lines.push("");
  lines.push(`- Date: ${today}`);
  lines.push(`- Source layer: ${sourceId}`);
  lines.push(`- Observations loaded: ${observations.length}`);
  lines.push(`- State terms retained: ${stateTerms}`);
  lines.push(`- Overlay rows: ${rows.length}`);
  lines.push(`- Filtered terms: ${filteredCounts}`);
  lines.push(
    `- Observation windows: ${Object.entries(countObservationsByWindow(observations))
      .map(([label, count]) => `${label}=${count}`)
      .join(", ")}`,
  );
  lines.push("");

  appendDetails(
    lines,
    `Proposed overlay (${rows.length})`,
    rows.length === 0 ? ["No rows emitted."] : rows.map((row) => `- ${row.phrase} (${row.weight})`),
  );

  for (const [reason, values] of Object.entries(filtered)) {
    const body = [];
    for (const value of values.slice(0, 80)) {
      body.push(`- ${value}`);
    }
    if (values.length > 80) {
      body.push(`- ... ${values.length - 80} more`);
    }
    appendDetails(lines, `Filtered: ${reason} (${values.length})`, body.length ? body : ["No terms."]);
  }

  lines.push("");
  return `${lines.join("\n")}\n`;
}

function appendDetails(lines, summary, body) {
  lines.push(`<details>`);
  lines.push(`<summary>${summary}</summary>`);
  lines.push("");
  lines.push(...body);
  lines.push("");
  lines.push(`</details>`);
  lines.push("");
}

function countObservationsByWindow(observations) {
  const counts = {};
  for (const window of WINDOWS) {
    counts[window.label] = 0;
  }
  for (const observation of observations) {
    counts[observation.window_label || windowLabelForHours(observation.window_hours || 24)] ||= 0;
    counts[observation.window_label || windowLabelForHours(observation.window_hours || 24)] += 1;
  }
  return counts;
}

function writePhrases(file, rows) {
  const lines = ["# phrase\tweight\ttags"];
  for (const row of rows) {
    lines.push(`${row.phrase}\t${row.weight}\t${row.tags}`);
  }
  writeText(file, `${lines.join("\n")}\n`);
}

function dedupeByTerm(rows) {
  const byTerm = new Map();
  for (const row of rows) {
    const previous = byTerm.get(row.term);
    if (!previous || (row.traffic || 0) > (previous.traffic || 0)) {
      byTerm.set(row.term, row);
    }
  }
  return [...byTerm.values()].sort((left, right) => (right.traffic || 0) - (left.traffic || 0) || left.term.localeCompare(right.term));
}

function normalizeTerm(value) {
  let term = String(value || "").normalize("NFKC").replace(/\s+/g, "").trim();
  for (const [from, to] of PHRASE_REPLACEMENTS) {
    term = term.replaceAll(from, to);
  }
  term = Array.from(term)
    .map((character) => CHAR_REPLACEMENTS.get(character) || character)
    .join("");
  return term;
}

function hasAsciiAlnum(value) {
  return /[A-Za-z0-9]/.test(value);
}

function isHanOnly(value) {
  return /^[\p{Script=Han}]+$/u.test(value);
}

function numberOrNull(value) {
  const number = Number(value);
  return Number.isFinite(number) ? number : null;
}

function windowLabelForHours(hours) {
  const matched = WINDOWS.find((window) => window.hours === Number(hours));
  return matched ? matched.label : `${hours}h`;
}

function normalizeWindowLabel(label) {
  if (WINDOWS.some((window) => window.label === label)) {
    return label;
  }
  const hours = Number.parseInt(label, 10);
  return Number.isFinite(hours) ? windowLabelForHours(hours) : label;
}

function normalizeSeenWindows(value, fallbackSeenDates) {
  if (value && typeof value === "object" && !Array.isArray(value)) {
    const result = {};
    for (const [label, dates] of Object.entries(value)) {
      const normalizedLabel = normalizeWindowLabel(label);
      result[normalizedLabel] = Array.isArray(dates) ? dates.filter(isDateOnly) : [];
    }
    return result;
  }
  const seenDates = Array.isArray(fallbackSeenDates) ? fallbackSeenDates.filter(isDateOnly) : [];
  return seenDates.length ? { "24h": seenDates } : {};
}

function mapSeenWindowsToSets(value) {
  const result = {};
  for (const [label, dates] of Object.entries(value)) {
    result[label] = new Set(Array.isArray(dates) ? dates.filter(isDateOnly) : []);
  }
  return result;
}

function pruneSeenWindows(value, cutoff, today) {
  const result = {};
  for (const window of WINDOWS) {
    const dates = [...(value[window.label] || [])].filter((date) => date >= cutoff && date <= today);
    if (dates.length > 0) {
      result[window.label] = new Set(dates);
    }
  }
  return result;
}

function serializeSeenWindows(value) {
  const result = {};
  for (const window of WINDOWS) {
    const dates = [...(value[window.label] || [])].sort();
    if (dates.length > 0) {
      result[window.label] = dates;
    }
  }
  return result;
}

function requiredOption(options, key) {
  if (!options[key]) {
    throw new Error(`missing required option --${key}`);
  }
  return String(options[key]);
}

function listJsonFiles(root) {
  const files = [];
  if (!fs.existsSync(root)) {
    return files;
  }
  for (const entry of fs.readdirSync(root, { withFileTypes: true })) {
    const fullPath = path.join(root, entry.name);
    if (entry.isDirectory()) {
      files.push(...listJsonFiles(fullPath));
    } else if (entry.isFile() && entry.name.endsWith(".json")) {
      files.push(fullPath);
    }
  }
  return files.sort();
}

function writeJson(file, value) {
  writeText(file, `${JSON.stringify(value, null, 2)}\n`);
}

function writeText(file, text) {
  const parent = path.dirname(file);
  if (parent && parent !== ".") {
    fs.mkdirSync(parent, { recursive: true });
  }
  fs.writeFileSync(file, text);
}

function taipeiDate() {
  return new Intl.DateTimeFormat("en-CA", {
    timeZone: "Asia/Taipei",
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  }).format(new Date());
}

function dateOnly(value) {
  if (!value) {
    return null;
  }
  const match = String(value).match(/^\d{4}-\d{2}-\d{2}/);
  return match ? match[0] : null;
}

function isDateOnly(value) {
  return /^\d{4}-\d{2}-\d{2}$/.test(value);
}

function addDays(date, delta) {
  const parsed = new Date(`${date}T00:00:00Z`);
  parsed.setUTCDate(parsed.getUTCDate() + delta);
  return parsed.toISOString().slice(0, 10);
}

function daysBetween(start, end) {
  const startTime = Date.parse(`${start}T00:00:00Z`);
  const endTime = Date.parse(`${end}T00:00:00Z`);
  return Math.floor((endTime - startTime) / 86_400_000);
}

function minDate(left, right) {
  if (!left) {
    return right;
  }
  if (!right) {
    return left;
  }
  return left < right ? left : right;
}

function maxDate(left, right) {
  if (!left) {
    return right;
  }
  if (!right) {
    return left;
  }
  return left > right ? left : right;
}

function countSeenSince(seenDates, cutoff) {
  return [...seenDates].filter((date) => date >= cutoff).length;
}

main().catch((error) => {
  console.error(error?.stack || error);
  process.exit(1);
});
