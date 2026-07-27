#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const OUT_DIR = process.env.MISSING_WORD_OUT_DIR ?? path.join(ROOT, ".github/missing-word");
const NORMALIZED_PATH = path.join(ROOT, "normalized/smart-mandarin.tsv");

const REQUIRED_FIELDS = {
  phrase: ["詞語", "Phrase"],
  keyboard: ["注音鍵盤碼", "Keyboard Zhuyin"],
};

async function main() {
  fs.mkdirSync(OUT_DIR, { recursive: true });

  const issueBody = process.env.ISSUE_BODY ?? "";
  const issueNumber = process.env.ISSUE_NUMBER ?? "";
  const issueUser = process.env.ISSUE_USER ?? "";
  const fields = parseIssueForm(issueBody);
  const phrase = readField(fields, REQUIRED_FIELDS.phrase);
  const keyboard = readField(fields, REQUIRED_FIELDS.keyboard);

  if (!phrase || !keyboard) {
    writeComment([
      "您好！感謝您回報缺詞！",
      "但目前無法處理這份缺詞回報，因為 issue 內容缺少「詞語」或「注音鍵盤碼」。",
      "",
      "請使用「缺詞回報」issue template，或編輯 issue 補上這兩個欄位。",
    ].join("\n"));
    setOutput("should_pr", "false");
    return;
  }

  const dryRun = runNodeScript("scripts/lexicon/add-explicit.mjs", [keyboard, phrase, "--dry-run"]);
  const qstring = matchLine(dryRun, /^qstring:\s*(.+)$/m);
  const bpmf = matchLine(dryRun, /^bpmf:\s*(.+)$/m);
  const weight = matchLine(dryRun, /^weight:\s*(.+)$/m);

  const existingRows = findExistingRows(phrase, qstring);
  if (existingRows.length > 0) {
    const explain = runNodeScript("scripts/audit/explain-weight.mjs", [phrase]);
    writeComment([
      "您好！感謝您回報缺詞！",
      `但「${phrase}」已存在於目前詞庫中，請至「偏好設定」->「更新」中檢查是否有新版詞庫。`,
      "",
      `- 注音：${bpmf}`,
      `- qstring：\`${qstring}\``,
      "",
      "目前找到的 row：",
      "",
      ...existingRows.map(
        (row) =>
          `- \`${row.qstring}\` ${row.phrase}，權重 \`${row.weight}\`，來源 \`${row.sourceId}\`，tags \`${row.tags}\``,
      ),
      "",
      "<details>",
      "<summary>權重與來源檢查輸出</summary>",
      "",
      "```text",
      explain.trim(),
      "```",
      "",
      "</details>",
    ].join("\n"));
    setOutput("should_pr", "false");
    return;
  }

  const addOutput = runNodeScript("scripts/lexicon/add-explicit.mjs", [keyboard, phrase]);
  const row = matchLine(addOutput, /^Appended to .+\n(.+)$/m);
  const prTitle = `Add missing word: ${phrase}`;
  const commitMessage = `feat: add missing word ${phrase}`;

  writeComment([
    "您好！感謝您回報缺詞！",
    `經過自動檢查後，已確認「${phrase}」尚未存在於目前詞庫中。`,
    "",
    "已自動產生補詞變更，後續將會由維護者審查。",
    "",
    `- 回報者：@${issueUser}`,
    `- 注音：${bpmf}`,
    `- qstring：\`${qstring}\``,
    `- 建議權重：${weight}`,
    `- 新增 row：\`${row}\``,
  ].join("\n"));

  fs.writeFileSync(
    path.join(OUT_DIR, "pr-body.md"),
    [
      `Closes #${issueNumber}`,
      "",
      "由缺詞回報 issue 自動產生。",
      "",
      `- 詞語：${phrase}`,
      `- 注音：${bpmf}`,
      `- 鍵盤碼：\`${keyboard}\``,
      `- qstring：\`${qstring}\``,
      `- 建議權重：${weight}`,
      `- 新增 row：\`${row}\``,
      "",
      "審查時請確認讀音、詞形與權重是否符合預期。",
    ].join("\n"),
    "utf8",
  );

  setOutput("should_pr", "true");
  setOutput("pr_title", prTitle);
  setOutput("commit_message", commitMessage);
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

function findExistingRows(phrase, qstring) {
  if (!fs.existsSync(NORMALIZED_PATH)) {
    throw new Error(`normalized lexicon not found: ${NORMALIZED_PATH}`);
  }

  return fs
    .readFileSync(NORMALIZED_PATH, "utf8")
    .split(/\r?\n/)
    .filter(Boolean)
    .map((line) => {
      const [rowQstring, rowPhrase, weight, sourceId, tags = ""] = line.split("\t");
      return { qstring: rowQstring, phrase: rowPhrase, weight, sourceId, tags };
    })
    .filter((row) => row.phrase === phrase && row.qstring === qstring);
}

function runNodeScript(script, args) {
  return execFileSync(process.execPath, [script, ...args], {
    cwd: ROOT,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
    env: { ...process.env, NORMALIZED_PATH },
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
    "無法自動處理這份缺詞回報。",
    "",
    "請檢查「詞語」與「注音鍵盤碼」是否正確，或等待維護者手動確認。",
    "",
    "```text",
    detail,
    "```",
  ].join("\n"));
  setOutput("should_pr", "false");
  process.exitCode = 1;
});
