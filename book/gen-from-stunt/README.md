Very smol script to generate an mdbook from the ToastStunt programmers manual.

All this does is: it breaks apart the monolithic `.md` file into a structured mdBook; each level 2 heading (`##`) gets its own page.

# Hacking setup

```
cd moor/book/gen-from-stunt
python -m venv .venv
. .venv/bin/activate
pip install -r requirements.txt
```

# Update Toast manual

```
./fetch-stunt-manual.sh
git add ./toaststunt-programmers-manual.md
```

# Execute

**WATCH OUT**: This completely, without asking any questions, truncates and rewrites `../src`. You _will_ lose any uncommitted manual changes.

```
./main.py
```
