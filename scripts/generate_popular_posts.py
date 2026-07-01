#!/usr/bin/env python3
"""
Generate _data/popular_posts.yml from a Google Search Console Pages CSV export.

Usage:
    python3 scripts/generate_popular_posts.py csv/Pages.csv
"""

import csv
import os
import re
import sys
from urllib.parse import urlparse


def parse_posts(posts_dir):
    """Build a map of URL path (normalized) -> {year, title} from _posts/ front matter."""
    permalink_map = {}
    for fname in os.listdir(posts_dir):
        if not fname.endswith(".md"):
            continue
        # Extract year from filename (YYYY-MM-DD-slug.md)
        m = re.match(r"(\d{4})-\d{2}-\d{2}-.+\.md", fname)
        if not m:
            continue
        year = int(m.group(1))

        filepath = os.path.join(posts_dir, fname)
        title = None
        permalink = None
        in_front_matter = False
        with open(filepath, encoding="utf-8") as f:
            for line in f:
                stripped = line.strip()
                if stripped == "---":
                    if not in_front_matter:
                        in_front_matter = True
                        continue
                    else:
                        break  # end of front matter
                if in_front_matter:
                    if stripped.startswith("title:"):
                        title = stripped[len("title:"):].strip().strip('"').strip("'")
                    elif stripped.startswith("permalink:"):
                        permalink = stripped[len("permalink:"):].strip().strip('"').strip("'")

        if permalink and title:
            # Normalize: strip trailing slash
            norm = permalink.rstrip("/")
            if not norm.startswith("/"):
                norm = "/" + norm
            permalink_map[norm] = {"year": year, "title": title}

    return permalink_map


def parse_csv(csv_path):
    """Parse Google Search Console Pages CSV, return list of {url, clicks}."""
    rows = []
    with open(csv_path, encoding="utf-8") as f:
        reader = csv.DictReader(f)
        for row in reader:
            url = row.get("Top pages", "")
            clicks = int(row.get("Clicks", 0))
            if clicks > 0 and url:
                rows.append({"url": url, "clicks": clicks})
    return rows


def main():
    if len(sys.argv) < 2:
        print(f"Usage: {sys.argv[0]} <Pages.csv>", file=sys.stderr)
        sys.exit(1)

    csv_path = sys.argv[1]
    repo_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    posts_dir = os.path.join(repo_root, "_posts")

    permalink_map = parse_posts(posts_dir)
    csv_rows = parse_csv(csv_path)

    # Match CSV URLs to posts
    all_posts = []
    for row in csv_rows:
        path = urlparse(row["url"]).path.rstrip("/")
        if not path.startswith("/"):
            path = "/" + path
        if path in permalink_map:
            info = permalink_map[path]
            all_posts.append({
                "title": info["title"],
                "url": path + "/",
                "clicks": row["clicks"],
            })

    # Top 3 by clicks
    top = sorted(all_posts, key=lambda e: e["clicks"], reverse=True)[:3]

    output_path = os.path.join(repo_root, "_data", "popular_posts.yml")
    with open(output_path, "w", encoding="utf-8") as f:
        for entry in top:
            safe_title = entry["title"].replace('"', '\\"')
            f.write(f'- title: "{safe_title}"\n')
            f.write(f'  url: "{entry["url"]}"\n')
            f.write(f"  clicks: {entry['clicks']}\n")

    print(f"Generated {output_path}")
    for e in top:
        print(f"  {e['clicks']:>5} clicks  {e['title']}")


if __name__ == "__main__":
    main()
