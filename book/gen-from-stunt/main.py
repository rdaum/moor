#!/usr/bin/env python3

import sys
import logging
from typing import Iterable
from dataclasses import dataclass
from pathlib import Path
from slugify import slugify
import shutil
import io
import re


STUNT_MANUAL: Path = Path("./toaststunt-programmers-manual.md")
SRC: Path = Path("../src")
HEADER_RE: re.Pattern = re.compile("^#+ ")

# Generate separate pages for direct children of these sections
EXPLODE_SUBSECTIONS: list[str] = [
    "The ToastStunt Database",
    "The Built-in Command Parser",
    "The MOO Programming Language",
    "Built-in Functions",
]

def main() -> int | None:
    logging.basicConfig()
    shutil.rmtree(SRC)
    SRC.mkdir()

    summary_lines = []
    summary_suffix_lines = []

    current_path: Path = Path("./")
    current_file: io.StringIO | None = None

    sections = gen_sections()

    for section in sections:
        if section.level == 1 or (section.parent_title in EXPLODE_SUBSECTIONS):
            if current_file is not None:
                current_file.close()

            if "Written for ToastStunt" in section.title:
                section.title = "Legal"
            if section.title == "Table of Contents":
                continue

            current_path = section.path
            (SRC / current_path).parent.mkdir(parents=True, exist_ok=True)
            current_file = (SRC / current_path).open("w")

            link = f"./{current_path}"

            summary_line = f"{" " * 4 * (section.level - 1)}- [{section.title}]({link})\n"
            if section.title == "Legal":
                summary_suffix_lines.append(summary_line)
            else:
                summary_lines.append(summary_line)

        if current_file:
            current_file.write(f"{"#" * section.level} {section.title}\n")
            current_file.writelines(section.lines)

    with open(SRC / "SUMMARY.md", "w") as summary:
        summary.write("# Summary\n\n")
        summary.writelines(summary_lines)
        summary.write("\n--------\n")
        summary.writelines(summary_suffix_lines)
        

@dataclass
class StuntSection:
    title_path: list[str]
    lines: list[str]

    @property
    def title(self) -> str:
        return self.title_path[-1]

    @title.setter
    def title(self, value: str) -> None:
        self.title_path[-1] = value

    @property
    def level(self) -> int:
        return len(self.title_path)

    @property
    def parent_title(self) -> str | None:
        try:
            return self.title_path[-2]
        except IndexError:
            return None

    @property
    def path(self) -> Path:
        path = Path("./")
        for p in self.title_path:
            path /= slugify(p)
        return path.with_suffix(".md")

def gen_sections() -> Iterable[StuntSection]:
    current_section: StuntSection | None = None
    title_stack: list[str] = []

    with open(STUNT_MANUAL, "r") as stunt:
        for line in stunt:
            if HEADER_RE.match(line):
                if current_section is not None:
                    yield current_section

                fence = line.split(" ", 1)[0]
                level = len(fence)
                assert fence.count("#") == level, fence

                title_stack = title_stack[:(level-1)]
                title = line.lstrip("# ").rstrip()
                title_stack.append(title)

                current_section = StuntSection(
                    title_path = title_stack[1:],
                    lines = [],
                )
                

            elif current_section is not None:
                current_section.lines.append(line)


if __name__ == "__main__":
    sys.exit(main())