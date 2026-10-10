#!/usr/bin/env python3
"""Render the Portuguese Markdown handoff; ReportLab is a separate docs dependency."""
from __future__ import annotations

import html
import os
from pathlib import Path
import re

from reportlab.lib import colors
from reportlab.lib.enums import TA_LEFT
from reportlab.lib.pagesizes import A4
from reportlab.lib.styles import ParagraphStyle
from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.ttfonts import TTFont
from reportlab.platypus import (
    BaseDocTemplate, Frame, PageBreak, PageTemplate, Paragraph, Spacer, Table,
    TableStyle,
)

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "docs/pt-BR/operations/handoff-marco-1.md"
OUTPUT = ROOT / "HANDOFF_MARCO_1.pdf"
NAVY = colors.HexColor("#18354a")
TEAL = colors.HexColor("#087d8b")
INK = colors.HexColor("#253544")
PALE = colors.HexColor("#f0f5f7")


def register_fonts() -> None:
    candidates = [
        Path(os.environ.get("HANDOFF_FONT_DIR", "/usr/share/fonts/truetype/dejavu")),
        Path("/System/Library/Fonts/Supplemental"),
    ]
    for folder in candidates:
        for names in [
            ("DejaVuSans.ttf", "DejaVuSans-Bold.ttf", "DejaVuSansMono.ttf"),
            ("Arial.ttf", "Arial Bold.ttf", "Andale Mono.ttf"),
        ]:
            if all((folder / name).is_file() for name in names):
                for face, name in zip(("Body", "Bold", "Mono"), names):
                    pdfmetrics.registerFont(TTFont(face, str(folder / name)))
                pdfmetrics.registerFontFamily("Body", normal="Body", bold="Bold")
                return
    raise SystemExit("Install Arial/DejaVu TTF fonts or configure HANDOFF_FONT_DIR")


def inline(text: str) -> str:
    """Escape Markdown into a safe, small ReportLab paragraph subset."""
    pattern = re.compile(r"`([^`]+)`|\*\*([^*]+)\*\*|\[([^]]+)\]\(([^)]+)\)")
    pieces = []
    position = 0
    for match in pattern.finditer(text):
        pieces.append(html.escape(text[position:match.start()]))
        code, bold, label, url = match.groups()
        if code is not None:
            pieces.append(f'<font name="Mono" size="8.7">{html.escape(code)}</font>')
        elif bold is not None:
            pieces.append(f'<b>{html.escape(bold)}</b>')
        else:
            # File references remain readable labels; web references are live links.
            if url.startswith(("https://", "http://")):
                pieces.append(f'<link href="{html.escape(url, quote=True)}" color="#087d8b">{html.escape(label)}</link>')
            else:
                pieces.append(f'<font color="#087d8b">{html.escape(label)}</font>')
        position = match.end()
    pieces.append(html.escape(text[position:]))
    return "".join(pieces)


def page_chrome(canvas, document) -> None:
    width, height = A4
    canvas.saveState()
    canvas.setStrokeColor(TEAL)
    canvas.setLineWidth(1.2)
    canvas.line(48, height - 40, width - 48, height - 40)
    canvas.setFillColor(NAVY)
    canvas.setFont("Bold", 8)
    canvas.drawString(48, height - 31, "RELIABLE EVENT RELAY")
    canvas.setFont("Body", 8)
    canvas.drawRightString(width - 48, height - 31, "HANDOFF | MARCO 1")
    canvas.setStrokeColor(colors.HexColor("#c9d7df"))
    canvas.setLineWidth(0.5)
    canvas.line(48, 40, width - 48, 40)
    canvas.setFillColor(INK)
    canvas.setFont("Body", 8)
    canvas.drawString(48, 27, "08/10/2026 | Base 7cba38d | Fechamento não commitado")
    canvas.drawRightString(width - 48, 27, f"Página {document.page}")
    canvas.restoreState()


def render() -> None:
    register_fonts()
    body = ParagraphStyle("body", fontName="Body", fontSize=10.1, leading=14.4,
                          textColor=INK, spaceAfter=8, alignment=TA_LEFT)
    heading = ParagraphStyle("heading", parent=body, fontName="Bold", fontSize=15,
                             leading=19, textColor=NAVY, spaceBefore=9,
                             spaceAfter=10, keepWithNext=True)
    title = ParagraphStyle("title", parent=heading, fontSize=27, leading=32,
                           spaceBefore=5, spaceAfter=14)
    cell = ParagraphStyle("cell", parent=body, fontSize=9.1, leading=12.4,
                          spaceAfter=0, splitLongWords=True)
    header = ParagraphStyle("table_header", parent=cell, fontName="Bold",
                            textColor=colors.white)
    bullet = ParagraphStyle("bullet", parent=body, leftIndent=10, firstLineIndent=-8)
    width, height = A4
    content_width = width - 96
    document = BaseDocTemplate(
        str(OUTPUT), pagesize=A4, leftMargin=48, rightMargin=48,
        topMargin=55, bottomMargin=53, title="Handoff do Marco 1 - Reliable Event Relay",
        author="Reliable Event Relay", subject="Revisão, validação e semântica do modelo durável de eventos",
        pageCompression=1,
    )
    frame = Frame(48, 53, content_width, height - 108,
                  leftPadding=0, rightPadding=0, topPadding=0, bottomPadding=0)
    document.addPageTemplates(PageTemplate(id="handoff", frames=[frame], onPage=page_chrome))
    lines = SOURCE.read_text(encoding="utf-8").splitlines()
    story = []
    index = 0
    while index < len(lines):
        line = lines[index].strip()
        if not line:
            index += 1
            continue
        if line == "<!-- pagebreak -->":
            story.append(PageBreak())
            index += 1
        elif line.startswith("# "):
            story.append(Paragraph(inline(line[2:]), title))
            index += 1
        elif line.startswith("## "):
            story.append(Paragraph(inline(line[3:]), heading))
            index += 1
        elif line.startswith("| "):
            raw_rows = []
            while index < len(lines) and lines[index].strip().startswith("|"):
                row = [c.strip() for c in lines[index].strip().strip("|").split("|")]
                if not all(re.fullmatch(r":?-+:?", c) for c in row):
                    raw_rows.append(row)
                index += 1
            count = len(raw_rows[0])
            ratios = [0.43, 0.57] if count == 2 else [0.23, 0.37, 0.40]
            # Validation commands need more space; avoid tiny command text.
            if raw_rows[0][0] == "Comando / verificação":
                ratios = [0.64, 0.36]
            rows = [[Paragraph(inline(c), header if r == 0 else cell)
                     for c in row] for r, row in enumerate(raw_rows)]
            table = Table(rows, colWidths=[content_width * r for r in ratios],
                          repeatRows=1, hAlign="LEFT")
            table.setStyle(TableStyle([
                ("BACKGROUND", (0, 0), (-1, 0), NAVY),
                ("ROWBACKGROUNDS", (0, 1), (-1, -1), [PALE, colors.white]),
                ("VALIGN", (0, 0), (-1, -1), "TOP"),
                ("LEFTPADDING", (0, 0), (-1, -1), 8),
                ("RIGHTPADDING", (0, 0), (-1, -1), 8),
                ("TOPPADDING", (0, 0), (-1, -1), 7),
                ("BOTTOMPADDING", (0, 0), (-1, -1), 7),
                ("LINEBELOW", (0, 0), (-1, 0), 0.7, TEAL),
                ("LINEBELOW", (0, 1), (-1, -1), 0.3, colors.HexColor("#d9e3e9")),
            ]))
            story.extend([table, Spacer(1, 10)])
        elif line.startswith("- "):
            story.append(Paragraph("- " + inline(line[2:]), bullet))
            index += 1
        else:
            paragraph = [line]
            index += 1
            while index < len(lines) and lines[index].strip() and not lines[index].strip().startswith(("#", "|", "- ", "<!--")):
                paragraph.append(lines[index].strip())
                index += 1
            story.append(Paragraph(inline(" ".join(paragraph)), body))
    document.build(story)
    print(f"Generated {OUTPUT.name}")


if __name__ == "__main__":
    render()
