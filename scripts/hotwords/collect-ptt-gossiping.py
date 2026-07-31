#!/usr/bin/env python3

"""Collect the last 24 hours of popular Gossiping posts with minimal output."""

import argparse
import json
import re
import sys
from collections import Counter
from datetime import datetime, timedelta
from pathlib import Path
from zoneinfo import ZoneInfo

import requests
from bs4 import BeautifulSoup
from requests.adapters import HTTPAdapter
from urllib3.util.retry import Retry


PTT_URL = "https://www.ptt.cc"
BOARD = "Gossiping"
TIMEZONE = ZoneInfo("Asia/Taipei")
INDEX_PAGE_LIMIT = 100
PTT_MIN_NET_PUSH = 20
COMMENT_MIN_ARTICLES = 10
COMMENT_MIN_PUSHES = 15
COMMENT_STOP_CHARS = set("的是在與又而了嗎呢不之很就也都讓被把對及或但從到為以要會有沒")
CCHAT_COMMENT_BANNED_PARTS = ("抽", "井", "池", "期望", "下一", "這", "新制", "直接", "大小", "課金", "限定")


def main():
    global BOARD
    parser = argparse.ArgumentParser()
    parser.add_argument("--crawler-dir", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--hours", type=int, default=24)
    parser.add_argument("--board", default=BOARD)
    parser.add_argument("--source")
    args = parser.parse_args()
    BOARD = args.board
    source = args.source or f"ptt-{BOARD.lower().replace('_', '')}"

    sys.path.insert(0, args.crawler_dir)
    from PttWebCrawler import crawler
    PttWebCrawler = crawler.PttWebCrawler

    now = datetime.now(TIMEZONE)
    cutoff = now - timedelta(hours=args.hours)
    session = create_session()
    session.cookies.set("over18", "1", domain="www.ptt.cc")
    # The upstream crawler calls requests.get directly. Route those requests through
    # the same session so article requests use the cookie, browser headers and retry policy.
    crawler.requests.get = session.get
    latest_page = get_latest_page(session)
    candidates = []
    pages_scanned = 0
    skipped = {"missing_date": 0, "outside_window": 0, "missing_title": 0, "net_push_below_threshold": 0}

    for page in range(latest_page, max(0, latest_page - INDEX_PAGE_LIMIT), -1):
        pages_scanned += 1
        entries = get_index_entries(session, page)
        if not entries:
            continue
        for entry in entries:
            if entry["score"] >= PTT_MIN_NET_PUSH:
                candidates.append(entry)
        if all(entry["published_date"] < cutoff.date() for entry in entries):
            break

    articles = []
    comment_terms = {}
    for candidate in candidates:
        parsed = json.loads(PttWebCrawler.parse(candidate["url"], candidate["article_id"], BOARD))
        published_at = parse_article_date(parsed.get("date"))
        net_push = parsed.get("message_count", {}).get("count", 0)
        if not published_at:
            skipped["missing_date"] += 1
            continue
        if published_at < cutoff or published_at > now:
            skipped["outside_window"] += 1
            continue
        if not parsed.get("article_title"):
            skipped["missing_title"] += 1
            continue
        if not isinstance(net_push, int) or net_push < PTT_MIN_NET_PUSH:
            skipped["net_push_below_threshold"] += 1
            continue
        articles.append(
            {
                "article_id": parsed["article_id"],
                "article_title": parsed["article_title"],
                "published_at": published_at.isoformat(),
                "message_count": {"count": net_push},
            }
        )
        collect_comment_terms(
            comment_terms,
            parsed.get("messages", []),
            parsed["article_id"],
            parsed["article_title"],
            net_push,
        )

    output = {
        "schema_version": 2,
        "source": source,
        "board": BOARD,
        "collected_at": now.isoformat(),
        "cutoff": cutoff.isoformat(),
        "index_pages_scanned": pages_scanned,
        "popular_articles_fetched": len(candidates),
        "articles_accepted": len(articles),
        "articles_skipped": skipped,
        "articles": articles,
        # Raw comments, account IDs and IP/time metadata are intentionally discarded.
        "comment_terms": serialize_comment_terms(comment_terms),
    }
    Path(args.output).parent.mkdir(parents=True, exist_ok=True)
    Path(args.output).write_text(json.dumps(output, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def create_session():
    session = requests.Session()
    session.headers.update(
        {
            "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            "Accept-Language": "zh-TW,zh;q=0.9,en;q=0.8",
            "User-Agent": (
                "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 "
                "(KHTML, like Gecko) Chrome/137.0.0.0 Safari/537.36"
            ),
        }
    )
    retry = Retry(
        total=3,
        connect=3,
        read=3,
        backoff_factor=1,
        status_forcelist=(429, 500, 502, 503, 504),
        allowed_methods=frozenset({"GET"}),
    )
    session.mount("https://", HTTPAdapter(max_retries=retry))
    return session


def get_latest_page(session):
    response = session.get(f"{PTT_URL}/bbs/{BOARD}/index.html", timeout=10)
    response.raise_for_status()
    page_numbers = []
    for link in BeautifulSoup(response.text, "html.parser").select("div.btn-group-paging a[href]"):
        match = re.search(rf"/bbs/{BOARD}/index(\d+)\.html$", link["href"])
        if match:
            page_numbers.append(int(match.group(1)))
    if not page_numbers:
        raise RuntimeError(f"Unable to determine the latest {BOARD} index page")
    return max(page_numbers) + 1


def get_index_entries(session, page):
    response = session.get(f"{PTT_URL}/bbs/{BOARD}/index{page}.html", timeout=10)
    response.raise_for_status()
    entries = []
    for entry in BeautifulSoup(response.text, "html.parser").select("div.r-ent"):
        link = entry.select_one("div.title a")
        date_text = entry.select_one("div.date")
        if not link or not date_text:
            continue
        published_date = parse_index_date(date_text.get_text(strip=True))
        if not published_date:
            continue
        href = link.get("href", "")
        article_id = href.rsplit("/", 1)[-1].removesuffix(".html")
        if not article_id:
            continue
        score_text = entry.select_one("div.nrec").get_text(strip=True) if entry.select_one("div.nrec") else ""
        entries.append(
            {
                "article_id": article_id,
                "url": f"{PTT_URL}{href}",
                "published_date": published_date,
                "score": index_score(score_text),
            }
        )
    return entries


def parse_index_date(value):
    match = re.fullmatch(r"(\d{1,2})/(\d{1,2})", value)
    if not match:
        return None
    now = datetime.now(TIMEZONE).date()
    month, day = map(int, match.groups())
    year = now.year
    if (month, day) > (now.month, now.day):
        year -= 1
    try:
        return datetime(year, month, day, tzinfo=TIMEZONE).date()
    except ValueError:
        return None


def parse_article_date(value):
    if not value:
        return None
    try:
        return datetime.strptime(value.strip(), "%a %b %d %H:%M:%S %Y").replace(tzinfo=TIMEZONE)
    except ValueError:
        return None


def index_score(value):
    if value == "爆":
        return 100
    try:
        return int(value)
    except ValueError:
        return 0


def collect_comment_terms(terms, messages, article_id, article_title, net_push):
    """Aggregate candidate phrases from positive pushes without retaining comment text."""
    for message in messages:
        if message.get("push_tag") != "推":
            continue
        pusher = message.get("push_userid")
        content = message.get("push_content", "")
        for sequence in re.findall(r"[\u4e00-\u9fff]{2,}", content):
            for start in range(len(sequence)):
                for length in range(2, min(4, len(sequence) - start) + 1):
                    term = sequence[start : start + length]
                    if not is_comment_candidate(term):
                        continue
                    value = terms.setdefault(
                        term,
                        {
                            "article_ids": set(),
                            "push_count": 0,
                            "pushers": set(),
                            "pushers_by_article": {},
                            "max_net_push": 0,
                            "title_aligned_article_ids": set(),
                        },
                    )
                    value["article_ids"].add(article_id)
                    value["push_count"] += 1
                    value["max_net_push"] = max(value["max_net_push"], net_push)
                    if pusher:
                        value["pushers"].add(pusher)
                        value["pushers_by_article"].setdefault(article_id, set()).add(pusher)
                    if title_aligns_with_term(article_title, term):
                        value["title_aligned_article_ids"].add(article_id)


def is_comment_candidate(term):
    return (
        len(term) >= 3
        and len(term) <= 4
        and len(set(term)) > 1
        and not any(character in COMMENT_STOP_CHARS for character in term)
        and (BOARD != "C_Chat" or not any(part in term for part in CCHAT_COMMENT_BANNED_PARTS))
    )


def title_aligns_with_term(title, term):
    title_characters = Counter(re.findall(r"[\u4e00-\u9fff]", title))
    return not (Counter(term) - title_characters)


def serialize_comment_terms(terms):
    qualified = []
    for term, value in terms.items():
        article_count = len(value["article_ids"])
        if BOARD == "C_Chat":
            qualifying_articles = [
                article_id
                for article_id, pushers in value["pushers_by_article"].items()
                if len(pushers) >= 2 and article_id in value["title_aligned_article_ids"]
            ]
            if article_count < 2 or not qualifying_articles:
                continue
        elif article_count < COMMENT_MIN_ARTICLES or value["push_count"] < COMMENT_MIN_PUSHES:
            continue
        qualified.append(
            {
                "term": term,
                "article_count": article_count,
                "push_count": value["push_count"],
                "distinct_pusher_count": len(value["pushers"]),
                "max_article_distinct_pusher_count": max(
                    (len(pushers) for pushers in value["pushers_by_article"].values()), default=0
                ),
                "max_net_push": value["max_net_push"],
            }
        )

    # Keep the longest supported phrase, so 「萊爾校長」 takes precedence over
    # overlapping fragments such as 「萊爾校」 and 「爾校長」.
    rows = []
    for row in qualified:
        has_stronger_superstring = any(
            row["term"] != other["term"]
            and row["term"] in other["term"]
            and other["article_count"] >= row["article_count"] * 0.8
            and other["push_count"] >= row["push_count"] * 0.8
            for other in qualified
        )
        if not has_stronger_superstring:
            rows.append(row)
    return sorted(rows, key=lambda row: (-row["article_count"], -row["push_count"], row["term"]))


if __name__ == "__main__":
    main()
