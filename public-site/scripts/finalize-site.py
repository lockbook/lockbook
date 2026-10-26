#!/usr/bin/env python3
"""Add chapter metadata and merge mdBook's pages into Zola's sitemap."""

import argparse
import html
from html.parser import HTMLParser
import json
from pathlib import Path
import re
from urllib.parse import urljoin
import xml.etree.ElementTree as ET


SITEMAP_NS = "http://www.sitemaps.org/schemas/sitemap/0.9"
BEGIN = "<!-- Lockbook page metadata -->"
END = "<!-- End Lockbook page metadata -->"
UTILITIES = {"404.html", "print.html", "toc.html"}


class Head(HTMLParser):
    def __init__(self, source):
        super().__init__()
        self.in_head = False
        self.canonical = None
        self.meta = {}
        self.description_tag = None
        self.feed(source)

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if tag == "head":
            self.in_head = True
        if not self.in_head:
            return
        if tag == "link" and attrs.get("rel") == "canonical":
            self.canonical = attrs["href"]
        if tag == "meta":
            name = attrs.get("name", attrs.get("property"))
            self.meta[name] = attrs.get("content", "")
            if name == "description":
                self.description_tag = self.get_starttag_text()

    def handle_endtag(self, tag):
        if tag == "head":
            self.in_head = False


def update_chapter(source, metadata, canonical, image, image_alt):
    # Re-running the finalizer must not duplicate metadata.
    source = re.sub(re.escape(BEGIN) + ".*?" + re.escape(END) + "\n?", "", source, flags=re.S)
    head = Head(source)
    if not head.description_tag:
        raise ValueError("mdBook output is missing its description tag")
    title = metadata["title"]
    if title != "Lockbook Documentation":
        title += " — Lockbook Documentation"
    title = html.escape(title, quote=True)
    description = html.escape(metadata["description"], quote=True)
    source, count = re.subn(r"<title>.*?</title>", lambda _: "<title>" + title + "</title>", source, count=1, flags=re.S)
    if count != 1:
        raise ValueError("mdBook output is missing its title tag")
    source = source.replace(head.description_tag, '<meta name="description" content="' + description + '">', 1)
    canonical, image, image_alt = (html.escape(value, quote=True) for value in (canonical, image, image_alt))
    tags = [
        BEGIN,
        '<link rel="canonical" href="' + canonical + '">',
        '<meta property="og:site_name" content="Lockbook">',
        '<meta property="og:type" content="website">',
        '<meta property="og:title" content="' + title + '">',
        '<meta property="og:description" content="' + description + '">',
        '<meta property="og:url" content="' + canonical + '">',
        '<meta property="og:image" content="' + image + '">',
        '<meta property="og:image:alt" content="' + image_alt + '">',
        '<meta name="twitter:card" content="summary_large_image">',
        '<meta name="twitter:title" content="' + title + '">',
        '<meta name="twitter:description" content="' + description + '">',
        '<meta name="twitter:image" content="' + image + '">',
        '<meta name="twitter:image:alt" content="' + image_alt + '">',
        END,
    ]
    return source.replace("</head>", "\n".join(tags) + "\n</head>", 1)


def finalize(output, metadata_path):
    homepage = Head((output / "index.html").read_text(encoding="utf-8"))
    if not homepage.canonical:
        raise ValueError("The homepage must define the site's canonical URL")
    site_url = homepage.canonical
    image = homepage.meta["og:image"]
    image_alt = homepage.meta["og:image:alt"]
    metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
    doc_urls = set()
    chapters = sorted((output / "docs").rglob("*.html"))
    if not chapters:
        raise ValueError("Build the docs before finalizing the site")
    for chapter in chapters:
        relative = chapter.relative_to(output / "docs").as_posix()
        if relative in UTILITIES:
            continue
        # mdBook also publishes the first chapter as index.html.
        key = "index.html" if relative == "book.html" else relative
        if key not in metadata:
            raise ValueError("Add a title and description for " + relative + " to " + str(metadata_path))
        entry = metadata[key]
        canonical = urljoin(site_url, "docs/" if key == "index.html" else "docs/" + key)
        source = chapter.read_text(encoding="utf-8")
        chapter.write_text(update_chapter(source, entry, canonical, image, image_alt), encoding="utf-8")
        if entry.get("sitemap", True):
            doc_urls.add(canonical)

    sitemap_path = output / "sitemap.xml"
    tree = ET.parse(sitemap_path)
    root = tree.getroot()
    ET.register_namespace("", SITEMAP_NS)
    # Replace the docs portion on subsequent runs, retaining Zola's lastmod data.
    for entry in list(root):
        loc = entry.find("{" + SITEMAP_NS + "}loc")
        if loc is not None and loc.text.startswith(urljoin(site_url, "docs/")):
            root.remove(entry)
    for url in sorted(doc_urls):
        entry = ET.SubElement(root, "{" + SITEMAP_NS + "}url")
        ET.SubElement(entry, "{" + SITEMAP_NS + "}loc").text = url
    tree.write(sitemap_path, encoding="utf-8", xml_declaration=True)
    print("Finalized docs metadata and added " + str(len(doc_urls)) + " documentation URLs to sitemap.xml")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    finalize(args.output, Path(__file__).resolve().parents[1] / "docs-metadata.json")
