#!/usr/bin/env python3

"""Collect the last 24 hours of popular Gossiping posts with minimal output."""

import argparse
import json
import re
import sys
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


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--crawler-dir", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--hours", type=int, default=24)
    args = parser.parse_args()

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

    for page in range(latest_page, max(0, latest_page - INDEX_PAGE_LIMIT), -1):
        pages_scanned += 1
        entries = get_index_entries(session, page)
        if not entries:
            continue
        for entry in entries:
            if entry["score"] >= 50:
                candidates.append(entry)
        if all(entry["published_date"] < cutoff.date() for entry in entries):
            break

    articles = []
    for candidate in candidates:
        parsed = json.loads(PttWebCrawler.parse(candidate["url"], candidate["article_id"], BOARD))
        published_at = parse_article_date(parsed.get("date"))
        net_push = parsed.get("message_count", {}).get("count", 0)
        if (
            not published_at
            or published_at < cutoff
            or published_at > now
            or not isinstance(net_push, int)
            or net_push < 50
            or not parsed.get("article_title")
        ):
            continue
        articles.append(
            {
                "article_id": parsed["article_id"],
                "article_title": parsed["article_title"],
                "published_at": published_at.isoformat(),
                "message_count": {"count": net_push},
            }
        )

    output = {
        "schema_version": 1,
        "source": "ptt-gossiping",
        "board": BOARD,
        "collected_at": now.isoformat(),
        "cutoff": cutoff.isoformat(),
        "index_pages_scanned": pages_scanned,
        "popular_articles_fetched": len(candidates),
        "articles": articles,
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
    match = re.search(rf'href="/bbs/{BOARD}/index(\d+)\.html">‹', response.text)
    return int(match.group(1)) + 1 if match else 1


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


if __name__ == "__main__":
    main()
