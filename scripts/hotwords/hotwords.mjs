#!/usr/bin/env node

import fs from "node:fs";
import { execFileSync } from "node:child_process";
import path from "node:path";

const DEFAULT_SOURCE_ID = "chiaki-auto-hotwords-overlay";
const DEFAULT_GEO = "TW";
const DEFAULT_HL = "zh-TW";
const DEFAULT_RETENTION_DAYS = 90;
const DEFAULT_STATE_WINDOW_DAYS = 180;
const DEFAULT_STATE_RETENTION_DAYS = 180;
const MIN_EMIT_SIGNAL = 3;
const MAX_SEGMENT_DERIVATION_SURFACE_LENGTH = 8;
const DEFAULT_OPENCC_BINARY = "opencc";
const DEFAULT_OPENCC_CONFIG = "s2tw.json";
const PR_CHANGE_TABLE_LIMIT = 50;

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
const PTT_MIN_NET_PUSH = 20;
const PTT_GENERIC_TERMS = new Set(["公告", "新聞", "問卦", "爆卦", "快訊", "最老", "公開賽", "冠軍", "可能", "現在", "目前", "擬報"]);
const PTT_BOUNDARY_STOP_CHARS = new Set(["的", "是", "在", "與", "又", "而", "了", "嗎", "呢", "不", "之", "萬"]);
const PTT_PERSON_NAME_STOP_CHARS = new Set(["要", "會", "在", "的", "不", "很", "一", "這", "那", "又", "沒", "有", "被"]);
const PTT_SURNAMES = new Set("王李張劉陳楊黃趙周吳徐孫胡朱高林何郭馬羅梁宋鄭謝韓唐馮于董蕭程曹袁鄧許傅沈曾彭呂蘇盧蔣蔡賈丁魏薛葉阮余潘杜戴夏鍾汪田任姜范方石姚廖鄒熊金陸郝孔白崔康毛邱秦江史顧侯邵孟龍萬段雷錢湯尹黎易常武喬賀賴龔文牛".split(""));
const PTT_PERSON_FOLLOWERS = ["驚傳", "表示", "憶", "曝", "稱", "遭", "被", "籲", "談", "批", "喊"];

const USAGE = `Usage:
  node scripts/hotwords/hotwords.mjs collect --output tmp/hotwords-observations/DATE.json [--date YYYY-MM-DD] [--ptt-input FILE]
  node scripts/hotwords/hotwords.mjs refresh --observations-dir tmp/hotwords-observations --state sources/chiaki-auto-hotwords-overlay/state.json --output sources/chiaki-auto-hotwords-overlay/phrases.tsv --summary tmp/hotwords-summary.md [--today YYYY-MM-DD]
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
  const sourceStats = {};

  if (!options["skip-google"]) {
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
              source: "google-trends",
              window_hours: window.hours,
              window_label: window.label,
            }))
            .filter((row) => row.term && isHanOnly(row.term) && !hasAsciiAlnum(row.term)),
        ),
      );
    }
  }

  if (options["ptt-input"]) {
    const pttLexicon = loadLexicon(String(options.normalized || "normalized/smart-mandarin.tsv"), DEFAULT_SOURCE_ID);
    for (const file of String(options["ptt-input"]).split(",").filter(Boolean)) {
      const pttObservations = collectPttObservations(file, observedOn, pttLexicon);
      observations.push(...pttObservations.observations);
      fetchedRows[pttObservations.source] = pttObservations.fetchedArticleCount;
      sourceStats[pttObservations.source] = {
        articles_fetched: pttObservations.fetchedArticleCount,
        articles_accepted: pttObservations.articleCount,
        observations: pttObservations.observations.length,
        articles_skipped: pttObservations.skipped,
      };
    }
  }

  const payload = {
    schema_version: 2,
    source: "hotwords",
    geo,
    hl,
    windows: WINDOWS.map((window) => ({ label: window.label, hours: window.hours })),
    observed_on: observedOn,
    collected_at: collectedAt,
    fetched_rows_by_window: fetchedRows,
    fetched_rows: Object.values(fetchedRows).reduce((sum, value) => sum + value, 0),
    source_stats: sourceStats,
    observations,
  };

  writeJson(output, payload);
  console.log(
    `Collected ${observations.length} normalized hotword observations from ${payload.fetched_rows} source rows into ${output}`,
  );
}

function collectPttObservations(file, observedOn, lexicon) {
  const payload = JSON.parse(fs.readFileSync(file, "utf8"));
  const source = String(payload.source || "ptt-gossiping");
  const candidates = new Map();
  const articles = Array.isArray(payload.articles) ? payload.articles : [];
  for (const article of articles) {
    const netPush = numberOrNull(article.message_count?.count) || 0;
    if (netPush < PTT_MIN_NET_PUSH || !article.article_title) {
      continue;
    }
    for (const candidate of pttTitleCandidates(article.article_title, lexicon)) {
      const value = candidates.get(candidate.term) || { titleIds: new Set(), maxTraffic: 0, kinds: new Set() };
      value.titleIds.add(article.article_id);
      value.maxTraffic = Math.max(value.maxTraffic, netPush);
      value.kinds.add(candidate.kind);
      candidates.set(candidate.term, value);
    }
  }
  for (const candidate of Array.isArray(payload.comment_terms) ? payload.comment_terms : []) {
    const term = normalizeTerm(candidate.term);
    if (!isPttCommentCandidate(term, candidate, source) || lexicon.byPhrase.has(term)) continue;
    const value = candidates.get(term) || { titleIds: new Set(), maxTraffic: 0, kinds: new Set() };
    value.maxTraffic = Math.max(value.maxTraffic, numberOrNull(candidate.max_net_push) || 0);
    value.kinds.add("comment");
    candidates.set(term, value);
  }
  const observations = [];
  for (const [term, candidate] of candidates) {
    const trusted =
      candidate.kinds.has("quoted") ||
      candidate.kinds.has("person") ||
      candidate.kinds.has("landmark") ||
      candidate.kinds.has("comment");
    if (!trusted && candidate.titleIds.size < 2) {
      continue;
    }
    observations.push({
      term,
      traffic: candidate.maxTraffic,
      source,
      observed_on: observedOn,
      window_hours: 24,
      window_label: "24h",
    });
  }
  return {
    articleCount: articles.length,
    fetchedArticleCount: numberOrNull(payload.popular_articles_fetched) || articles.length,
    skipped: payload.articles_skipped || {},
    source,
    observations: dedupeByTerm(observations),
  };
}

function isPttCommentCandidate(term, candidate, source) {
  if (source === "ptt-cchat") {
    return isPttCandidate(term) && Array.from(term).length >= 3 && Number(candidate.max_article_distinct_pusher_count) >= 2;
  }
  return (
    isPttCandidate(term) &&
    Array.from(term).length >= 3 &&
    Number(candidate.article_count) >= 10 &&
    Number(candidate.push_count) >= 15
  );
}

function pttTitleCandidates(title, lexicon) {
  const candidates = new Set();
  const plainTitle = String(title).replace(/^\s*(?:Re:\s*)?(?:\[[^\]]*\]\s*)+/i, "");
  const add = (term, kind) => {
    const normalized = normalizeTerm(term);
    if (isPttCandidate(normalized)) candidates.add(`${kind}\0${normalized}`);
  };

  for (const quoted of plainTitle.matchAll(/[「“]([^」”]+)[」”]/gu)) {
    for (const sequence of quoted[1].matchAll(/[\p{Script=Han}]+/gu)) {
      for (const term of pttUnknownRuns(sequence[0], lexicon)) add(term, "quoted");
    }
  }
  for (const match of plainTitle.matchAll(/(?:台北|新北|桃園|台中|台南|高雄|基隆|新竹|嘉義|屏東|宜蘭|花蓮|台東)([\p{Script=Han}]{1,3}[橋路站溪山港島])/gu)) {
    add(match[1], "landmark");
  }
  for (const sequence of plainTitle.matchAll(/[\p{Script=Han}]{2,}/gu)) {
    const people = pttPersonCandidates(sequence[0]);
    for (const term of pttUnknownRuns(sequence[0], lexicon)) {
      if (people.some((person) => term.startsWith(person) && term !== person)) continue;
      add(term, "unknown");
    }
    for (const term of people) add(term, "person");
  }
  return [...candidates].map((value) => {
    const [kind, term] = value.split("\0");
    return { term, kind };
  });
}

function pttPersonCandidates(text) {
  const surnamePattern = [...PTT_SURNAMES].join("");
  const followerPattern = PTT_PERSON_FOLLOWERS.join("|");
  const expression = new RegExp(`([${surnamePattern}][\\p{Script=Han}]{1,2}?)(?=${followerPattern})`, "gu");
  return [...text.matchAll(expression)]
    .map((match) => match[1])
    .filter((term) => !PTT_PERSON_NAME_STOP_CHARS.has(Array.from(term)[1]));
}

function pttUnknownRuns(text, lexicon) {
  const tokens = tokenize(normalizeTerm(text), lexicon);
  const candidates = [];
  let current = "";
  for (const token of tokens) {
    if (Array.from(token).length === 1) {
      current += token;
    } else if (current) {
      candidates.push(current);
      current = "";
    }
  }
  if (current) candidates.push(current);
  return candidates;
}

function isPttCandidate(term) {
  const chars = Array.from(term);
  return (
    chars.length >= 2 &&
    chars.length <= 4 &&
    isHanOnly(term) &&
    !PTT_GENERIC_TERMS.has(term) &&
    !PTT_BOUNDARY_STOP_CHARS.has(chars[0]) &&
    !PTT_BOUNDARY_STOP_CHARS.has(chars.at(-1))
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
  const previousRows = loadOverlayRows(outputPath);
  const canonicalize = createOpenCcCanonicalizer({
    binary: String(options["opencc-binary"] || DEFAULT_OPENCC_BINARY),
    config: String(options["opencc-config"] || DEFAULT_OPENCC_CONFIG),
  });

  const observations = canonicalizeObservations(loadObservations(observationsDir), canonicalize);
  const state = canonicalizeState(loadState(statePath), canonicalize);
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
    previousRows,
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
      source: observation.source || payload.source || "google-trends",
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
      sources: Array.isArray(value.sources) && value.sources.length > 0 ? value.sources : ["google-trends"],
      admitted: value.admitted !== false,
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
      sources: new Set(value.sources || []),
      admitted: Boolean(value.admitted),
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
    if (entry.seen_dates.size === 0 || daysBetween(entry.last_seen, today) > DEFAULT_STATE_RETENTION_DAYS) {
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
      sources: new Set(),
      admitted: false,
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
  if (observation.source) {
    entry.sources.add(observation.source);
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
    covered_by_existing_core: [],
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
    const core = existingRecurringCore(term, state, lexicon);
    if (core) {
      filtered.covered_by_existing_core.push(`${term} (${core})`);
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
      watchlistTerms.add(term);
      filtered.expired.push(term);
      continue;
    }
    const signal = signalFor(entry, today);
    const newlyEligible = shouldEmitSignal(signal);
    if (!entry.admitted && !newlyEligible) {
      filtered.weak_signal.push(formatWeakSignal(term, signal));
      continue;
    }
    watchlistTerms.add(term);
    entry.admitted = true;
    const seenLast14 = countSeenSince(entry.seen_dates, addDays(today, -13));
    const seenLast30 = countSeenSince(entry.seen_dates, addDays(today, -29));
    const weight = weightFor({ daysSinceSeen, signal14: signal.score14, signal30: signal.score30 });
    const tags = [
      sourceId,
      "auto",
      `source=${[...entry.sources].sort().join("+") || "google-trends"}`,
      `first_seen=${entry.first_seen}`,
      `last_seen=${entry.last_seen}`,
      `seen_days_30=${seenLast30}`,
      `signal_14=${signal.score14}`,
      `signal_30=${signal.score30}`,
      `windows_14=${signal.windows14.join("+")}`,
      `max_traffic=${entry.max_traffic || 0}`,
      `status=${daysSinceSeen > 60 ? "dormant" : daysSinceSeen > 30 ? "cooling" : "active"}`,
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

function existingRecurringCore(term, state, lexicon) {
  const characters = Array.from(term);
  for (let length = Math.min(4, characters.length - 1); length >= 2; length -= 1) {
    const core = characters.slice(0, length).join("");
    if (!lexicon.byPhrase.has(core)) {
      continue;
    }
    const relatedTerms = [...state.keys()].filter((candidate) => candidate !== core && candidate.startsWith(core));
    if (relatedTerms.length >= 2) {
      return core;
    }
  }
  return null;
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
  if (daysSinceSeen > 60) {
    return "-3.0";
  }
  if (daysSinceSeen > 30) {
    return "-2.8";
  }
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
      admitted: Boolean(entry.admitted),
    };
    const derivedFrom = sortedDerivedFrom(entry);
    if (derivedFrom.length > 0) {
      terms[term].derived_from = derivedFrom.slice(0, 10);
    }
    terms[term].sources = [...(entry.sources || [])].sort();
  }
  return {
    schema_version: 2,
    updated_at: new Date().toISOString(),
    updated_on: today,
    retention_days: DEFAULT_RETENTION_DAYS,
    state_retention_days: DEFAULT_STATE_RETENTION_DAYS,
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

function buildSummary({ today, observations, rows, previousRows, filtered, stateTerms, sourceId }) {
  const lines = [];
  const changes = compareOverlayRows(previousRows, rows);
  const filteredCounts = Object.entries(filtered)
    .map(([reason, values]) => `${reason}=${values.length}`)
    .join(", ");

  lines.push(`# Auto Hotwords Refresh`);
  lines.push("");
  lines.push(`本次有 **${changes.behavioralCount} 項影響輸入排序的變動**：新增 ${changes.added.length}、移除 ${changes.removed.length}、權重調整 ${changes.weightChanged.length}。`);
  lines.push(`另有 ${changes.metadataChanged.length} 項僅更新觀測資料的詞彙，不影響排序。`);
  lines.push("");

  appendChangeTable(lines, "新增詞彙", changes.added, (row) => [row.phrase, row.weight, evidenceForRow(row)]);
  appendChangeTable(lines, "移除詞彙", changes.removed, (row) => [row.phrase, row.weight, removalReason(row.phrase, filtered)]);
  appendChangeTable(lines, "權重調整", changes.weightChanged, ({ previous, next }) => [
    next.phrase,
    `${previous.weight} → ${next.weight}`,
    evidenceForRow(next),
  ]);

  lines.push("## 收集摘要");
  lines.push("");
  lines.push(`- 日期：${today}`);
  lines.push(`- 資料層：${sourceId}`);
  lines.push(`- 載入觀測值：${observations.length}`);
  lines.push(`- 保留狀態詞：${stateTerms}`);
  lines.push(`- 覆蓋層詞數：${rows.length}`);
  lines.push(`- 篩除統計：${filteredCounts}`);
  lines.push(
    `- 觀測視窗：${Object.entries(countObservationsByWindow(observations))
      .map(([label, count]) => `${label}=${count}`)
      .join(", ")}`,
  );
  lines.push("");

  appendDetails(
    lines,
    `僅更新觀測資料、排序不變 (${changes.metadataChanged.length})`,
    changes.metadataChanged.length === 0
      ? ["沒有。"]
      : changes.metadataChanged.map(({ previous, next }) => `- ${next.phrase}：${previous.weight}（${evidenceForRow(next)}）`),
  );

  for (const [reason, values] of Object.entries(filtered)) {
    const body = [];
    for (const value of values.slice(0, 80)) {
      body.push(`- ${value}`);
    }
    if (values.length > 80) {
      body.push(`- ... ${values.length - 80} more`);
    }
    appendDetails(lines, `已篩除：${reason} (${values.length})`, body.length ? body : ["沒有。"]);
  }

  lines.push("");
  return `${lines.join("\n")}\n`;
}

function loadOverlayRows(file) {
  if (!fs.existsSync(file)) {
    return [];
  }
  return fs
    .readFileSync(file, "utf8")
    .split(/\r?\n/)
    .filter((line) => line && !line.startsWith("#"))
    .map((line) => {
      const [phrase, weight, tags] = line.split("\t");
      return { phrase, weight, tags: tags || "" };
    })
    .filter((row) => row.phrase && row.weight);
}

function compareOverlayRows(previousRows, nextRows) {
  const previousByPhrase = new Map(previousRows.map((row) => [row.phrase, row]));
  const nextByPhrase = new Map(nextRows.map((row) => [row.phrase, row]));
  const added = nextRows.filter((row) => !previousByPhrase.has(row.phrase));
  const removed = previousRows.filter((row) => !nextByPhrase.has(row.phrase));
  const weightChanged = [];
  const metadataChanged = [];

  for (const next of nextRows) {
    const previous = previousByPhrase.get(next.phrase);
    if (!previous) continue;
    if (previous.weight !== next.weight) {
      weightChanged.push({ previous, next });
    } else if (previous.tags !== next.tags) {
      metadataChanged.push({ previous, next });
    }
  }

  const byPhrase = (left, right) => left.phrase.localeCompare(right.phrase);
  return {
    added: added.sort(byPhrase),
    removed: removed.sort(byPhrase),
    weightChanged: weightChanged.sort((left, right) => byPhrase(left.next, right.next)),
    metadataChanged: metadataChanged.sort((left, right) => byPhrase(left.next, right.next)),
    behavioralCount: added.length + removed.length + weightChanged.length,
  };
}

function appendChangeTable(lines, title, entries, formatRow) {
  lines.push(`## ${title} (${entries.length})`);
  lines.push("");
  if (entries.length === 0) {
    lines.push("沒有。\n");
    return;
  }
  lines.push("| 詞彙 | 權重 | 訊號／原因 |");
  lines.push("| --- | --- | --- |");
  const visibleEntries = entries.slice(0, PR_CHANGE_TABLE_LIMIT);
  for (const entry of visibleEntries) {
    const [phrase, weight, detail] = formatRow(entry);
    lines.push(`| ${phrase} | ${weight} | ${detail} |`);
  }
  lines.push("");
  if (entries.length > visibleEntries.length) {
    appendDetails(
      lines,
      `${title}其餘 ${entries.length - visibleEntries.length} 項`,
      entries.slice(PR_CHANGE_TABLE_LIMIT).map((entry) => {
        const [phrase, weight, detail] = formatRow(entry);
        return `- ${phrase}｜${weight}｜${detail}`;
      }),
    );
  }
}

function evidenceForRow(row) {
  const tags = Object.fromEntries(
    row.tags.split(",").map((tag) => {
      const [key, value] = tag.split("=", 2);
      return [key, value || ""];
    }),
  );
  const details = [
    tags.source ? `來源 ${tags.source.replaceAll("+", "、")}` : null,
    tags.signal_30 ? `30 天訊號 ${tags.signal_30}` : null,
    tags.seen_days_30 ? `觀測 ${tags.seen_days_30} 天` : null,
    tags.last_seen ? `最近 ${tags.last_seen}` : null,
    tags.derived_from ? `派生自 ${tags.derived_from.replaceAll("+", "、")}` : null,
  ].filter(Boolean);
  return details.join("；") || "無額外訊號";
}

function removalReason(phrase, filtered) {
  const reasons = {
    expired: "超過保留期限",
    weak_signal: "訊號不足",
    existing_phrase: "已收錄於基底詞庫",
    covered_by_existing_core: "既有核心詞已涵蓋",
    typeable_by_top_segments: "可由高排名片段自然輸入",
    missing_character_reading: "無法推導讀音",
    query_like: "查詢型詞",
    too_short_or_long: "詞長不符合規則",
    non_han: "非全漢字詞",
  };
  for (const [reason, values] of Object.entries(filtered)) {
    if (values.some((value) => value === phrase || value.startsWith(`${phrase} (`))) {
      return reasons[reason] || "未通過目前篩選規則";
    }
  }
  return "未通過目前篩選規則";
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
  return String(value || "").normalize("NFKC").replace(/\s+/g, "").trim();
}

function createOpenCcCanonicalizer({ binary, config }) {
  return (terms) => {
    const input = terms.map(normalizeTerm).filter(Boolean);
    if (input.length === 0) {
      return [];
    }
    let output;
    try {
      output = execFileSync(binary, ["-c", config], {
        encoding: "utf8",
        input: `${input.join("\n")}\n`,
        maxBuffer: 16 * 1024 * 1024,
      });
    } catch (error) {
      throw new Error(`OpenCC ${binary} (${config}) is required when refreshing hotwords: ${error.message}`);
    }
    const converted = output.split(/\r?\n/).filter((line) => line.length > 0).map(normalizeTerm);
    if (converted.length !== input.length) {
      throw new Error(`OpenCC returned ${converted.length} terms for ${input.length} hotword terms`);
    }
    return converted.map((term) => term.replaceAll("臺灣", "台灣"));
  };
}

function canonicalizeObservations(observations, canonicalize) {
  const terms = canonicalize(observations.map((observation) => observation.term));
  return observations.map((observation, index) => ({ ...observation, term: terms[index] }));
}

function canonicalizeState(state, canonicalize) {
  const entries = [...state.entries()];
  const terms = canonicalize(entries.map(([term]) => term));
  const result = new Map();
  for (let index = 0; index < entries.length; index += 1) {
    const [, entry] = entries[index];
    const term = terms[index];
    const previous = result.get(term);
    if (!previous) {
      result.set(term, {
        first_seen: entry.first_seen,
        last_seen: entry.last_seen,
        seen_dates: new Set(entry.seen_dates),
        seen_windows: mapSeenWindowsToSets(entry.seen_windows),
        max_traffic: entry.max_traffic || 0,
        derived_from: new Set(entry.derived_from),
        sources: new Set(entry.sources),
        admitted: Boolean(entry.admitted),
      });
      continue;
    }
    previous.first_seen = minDate(previous.first_seen, entry.first_seen);
    previous.last_seen = maxDate(previous.last_seen, entry.last_seen);
    for (const date of entry.seen_dates) previous.seen_dates.add(date);
    for (const [window, dates] of Object.entries(entry.seen_windows)) {
      previous.seen_windows[window] ||= new Set();
      for (const date of dates) previous.seen_windows[window].add(date);
    }
    previous.max_traffic = Math.max(previous.max_traffic, entry.max_traffic || 0);
    for (const source of entry.derived_from) previous.derived_from.add(source);
    for (const source of entry.sources) previous.sources.add(source);
    previous.admitted ||= Boolean(entry.admitted);
  }
  return result;
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
