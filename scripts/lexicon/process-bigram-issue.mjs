#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { DatabaseSync } from "node:sqlite";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const OUT_DIR = process.env.BIGRAM_REPORT_OUT_DIR ?? path.join(ROOT, ".github/bigram-report");
const NORMALIZED_PATH = path.join(ROOT, "normalized/smart-mandarin.tsv");
const DB_PATH = process.env.DB_PATH ?? path.join(ROOT, "dist/dev/ChiaKeySource-dev.db");

const REQUIRED_FIELDS = {
  previous: ["正確前詞", "Previous word"],
  previousKeyboard: ["正確前詞注音鍵盤碼", "Previous keyboard Zhuyin"],
  current: ["正確後詞", "Current word"],
  currentKeyboard: ["正確後詞注音鍵盤碼", "Current keyboard Zhuyin"],
  context: ["補充說明", "Context"],
};

async function main() {
  fs.mkdirSync(OUT_DIR, { recursive: true });

  const issueBody = process.env.ISSUE_BODY ?? "";
  const issueNumber = process.env.ISSUE_NUMBER ?? "";
  const issueUser = process.env.ISSUE_USER ?? "";
  const fields = parseIssueForm(issueBody);
  const previous = readField(fields, REQUIRED_FIELDS.previous);
  const previousKeyboard = readField(fields, REQUIRED_FIELDS.previousKeyboard);
  const current = readField(fields, REQUIRED_FIELDS.current);
  const currentKeyboard = readField(fields, REQUIRED_FIELDS.currentKeyboard);
  const context = fields.get(REQUIRED_FIELDS.context[0]) ?? "";

  if (!previous || !previousKeyboard || !current || !currentKeyboard) {
    writeComment([
      "您好！感謝您回報長句選字錯誤！",
      "無法處理這份長句選字錯誤回報，因為 issue 內容缺少必要欄位。",
      "",
      "請使用「長句選字錯誤」issue template，或編輯 issue 補上正確前詞、前詞注音鍵盤碼、正確後詞與後詞注音鍵盤碼。",
    ].join("\n"));
    setOutput("should_pr", "false");
    return;
  }

  const dryRun = runNodeScript("scripts/lexicon/add-bigram.mjs", [
    previous,
    previousKeyboard,
    current,
    currentKeyboard,
    "--dry-run",
  ]);
  const previousBpmf = matchLine(dryRun, /^previous bpmf:\s*(.+)$/m);
  const previousQstring = matchLine(dryRun, /^previous qstring:\s*(.+)$/m);
  const currentBpmf = matchLine(dryRun, /^current bpmf:\s*(.+)$/m);
  const currentQstring = matchLine(dryRun, /^current qstring:\s*(.+)$/m);
  const combinedQstring = matchLine(dryRun, /^combined qstring:\s*(.+)$/m);
  const probability = matchLine(dryRun, /^probability:\s*(.+)$/m);

  const previousRows = findNormalizedRows(previous).filter((row) => row.qstring === previousQstring);
  const currentRows = findNormalizedRows(current).filter((row) => row.qstring === currentQstring);
  if (previousRows.length === 0 || currentRows.length === 0) {
    writeComment([
      "您好！感謝您回報長句選字錯誤！",
      "這份回報看起來還不能自動處理，因為 bigram 需要「前詞」與「後詞」都已經存在於詞庫。",
      "",
      previousRows.length === 0
        ? `- 找不到正確前詞與指定讀音：「${previous}」 / \`${previousQstring}\``
        : "",
      currentRows.length === 0
        ? `- 找不到正確後詞與指定讀音：「${current}」 / \`${currentQstring}\``
        : "",
      "",
      "如果詞庫中缺少前詞或後詞，請先使用「缺詞回報」回報該缺詞。",
      "如果缺的是一個不可拆的單詞，也請改用「缺詞回報」。如果兩個詞都應該已存在，請補充正確注音或等待維護者手動確認。",
    ].filter(Boolean).join("\n"));
    setOutput("should_pr", "false");
    return;
  }

  const existing = findExistingBigrams(previous, current, combinedQstring);
  if (existing.exact.length > 0) {
    writeComment([
      "您好！感謝您回報長句選字錯誤！",
      `經過自動檢查後，發現「${previous} -> ${current}」已存在於目前詞庫中，請至「偏好設定」->「更新」中檢查是否有新版詞庫。`,
      "",
      `- 目前正確前詞注音：${previousBpmf}`,
      `- 目前正確後詞注音：${currentBpmf}`,
      `- 目前 qstring：\`${combinedQstring}\``,
      "",
      "目前找到的 bigram：",
      "",
      ...existing.exact.map(
        (row) => `- \`${row.qstring}\` ${row.previous} -> ${row.current}，probability \`${row.probability}\``,
      ),
      competitionBlock(existing.competitors),
    ].filter(Boolean).join("\n"));
    setOutput("should_pr", "false");
    return;
  }

  const addOutput = runNodeScript("scripts/lexicon/add-bigram.mjs", [
    previous,
    previousKeyboard,
    current,
    currentKeyboard,
  ]);
  const row = matchLine(addOutput, /^Appended to .+\n(.+)$/m);
  const prTitle = `Add bigram: ${previous} -> ${current}`;
  const commitMessage = `feat: add bigram ${previous} -> ${current}`;

  writeComment([
    "您好！感謝您回報長句選字錯誤！",
    `經過自動檢查後，已確認「${previous} -> ${current}」尚未存在於目前詞庫中。`,
    "",
    "已自動產生 bigram 變更，後續將會由維護者審查。",
    "",
    `- 回報者：@${issueUser}`,
    `- bigram：${previous} -> ${current}`,
    `- 正確前詞注音：${previousBpmf}`,
    `- 正確後詞注音：${currentBpmf}`,
    `- qstring：\`${combinedQstring}\``,
    `- probability：\`${probability}\``,
    `- 新增 row：\`${row}\``,
    contextBlock(context),
    competitionBlock(existing.competitors),
  ].filter(Boolean).join("\n"));

  fs.writeFileSync(
    path.join(OUT_DIR, "pr-body.md"),
    [
      `Closes #${issueNumber}`,
      "",
      "由長句選字錯誤 issue 自動產生。",
      "",
      `- bigram：${previous} -> ${current}`,
      `- 正確前詞注音：${previousBpmf}`,
      `- 正確前詞鍵盤碼：\`${previousKeyboard}\``,
      `- 正確後詞注音：${currentBpmf}`,
      `- 正確後詞鍵盤碼：\`${currentKeyboard}\``,
      `- qstring：\`${combinedQstring}\``,
      `- probability：\`${probability}\``,
      `- 新增 row：\`${row}\``,
      contextBlock(context),
      "",
      "審查時請確認前後詞切分、前後詞讀音與 probability 是否符合預期。",
    ].join("\n"),
    "utf8",
  );

  setOutput("should_pr", "true");
  setOutput("pr_title", prTitle);
  setOutput("commit_message", commitMessage);
}

function contextBlock(context) {
  const trimmed = context.trim();
  if (!trimmed) return "";
  return ["", "補充說明：", "", trimmed].join("\n");
}

function parseIssueForm(body) {
  const fields = new Map();
  let currentName = null;
  let currentLines = [];

  const flush = () => {
    if (currentName) {
      fields.set(currentName, normalizeValue(currentLines.join("\n")));
    }
  };

  for (const line of body.split(/\r?\n/)) {
    const heading = line.match(/^###\s+(.+?)\s*$/);
    if (heading) {
      flush();
      currentName = heading[1].trim();
      currentLines = [];
    } else if (currentName) {
      currentLines.push(line);
    }
  }
  flush();

  return fields;
}

function normalizeValue(value) {
  const trimmed = value.trim();
  return trimmed === "_No response_" ? "" : trimmed;
}

function readField(fields, names) {
  for (const name of names) {
    const value = fields.get(name);
    if (value) return firstLine(value);
  }
  return "";
}

function firstLine(value) {
  return value
    .split(/\r?\n/)
    .map((line) => line.trim())
    .find(Boolean) ?? "";
}

function findNormalizedRows(phrase) {
  if (!fs.existsSync(NORMALIZED_PATH)) {
    throw new Error(`normalized lexicon not found: ${NORMALIZED_PATH}`);
  }

  return fs
    .readFileSync(NORMALIZED_PATH, "utf8")
    .split(/\r?\n/)
    .filter(Boolean)
    .map((line) => {
      const [qstring, rowPhrase, weight, sourceId, tags = ""] = line.split("\t");
      return { qstring, phrase: rowPhrase, weight, sourceId, tags };
    })
    .filter((row) => row.phrase === phrase);
}

function findExistingBigrams(previous, current, qstring) {
  if (!fs.existsSync(DB_PATH)) {
    throw new Error(`release DB not found: ${DB_PATH}`);
  }

  const db = new DatabaseSync(DB_PATH, { readOnly: true });
  try {
    const exact = db
      .prepare(
        "select qstring, previous, current, probability from bigrams where qstring = ? and previous = ? and current = ? order by probability desc",
      )
      .all(qstring, previous, current);
    const competitors = db
      .prepare(
        "select qstring, previous, current, probability from bigrams where qstring = ? and previous = ? order by probability desc limit 8",
      )
      .all(qstring, previous);
    return { exact, competitors };
  } finally {
    db.close();
  }
}

function competitionBlock(competitors) {
  const rows = competitors.filter((row) => row.current);
  if (rows.length === 0) return "";
  return [
    "",
    "同一前詞與後詞讀音的既有 bigram 排名：",
    "",
    ...rows.map(
      (row) => `- ${row.previous} -> ${row.current}，probability \`${row.probability}\``,
    ),
  ].join("\n");
}

function runNodeScript(script, args) {
  return execFileSync(process.execPath, [script, ...args], {
    cwd: ROOT,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });
}

function matchLine(output, regex) {
  const match = output.match(regex);
  return match ? match[1].trim() : "";
}

function writeComment(body) {
  fs.writeFileSync(path.join(OUT_DIR, "comment.md"), body, "utf8");
}

function setOutput(name, value) {
  const outputPath = process.env.GITHUB_OUTPUT;
  if (!outputPath) return;
  fs.appendFileSync(outputPath, `${name}=${escapeOutput(value)}\n`, "utf8");
}

function escapeOutput(value) {
  return String(value).replaceAll("%", "%25").replaceAll("\n", "%0A").replaceAll("\r", "%0D");
}

main().catch((error) => {
  fs.mkdirSync(OUT_DIR, { recursive: true });
  const detail = error.stderr?.toString?.().trim() || error.message;
  writeComment([
    "無法自動處理這份長句選字錯誤回報。",
    "",
    "請檢查欄位是否正確，或等待維護者手動確認。",
    "",
    "```text",
    detail,
    "```",
  ].join("\n"));
  setOutput("should_pr", "false");
  process.exitCode = 1;
});
