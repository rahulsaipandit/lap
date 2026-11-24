# CLAUDE.md

## Fork conventions (rahulsaipandit/lap, forked from julyx10/lap)

- This repo is a custom fork. Every change that is not upstream must carry a one-line code comment
  starting with `RP-CUSTOM` stating why (e.g. `// RP-CUSTOM: compare file_size as well as mtime`).
  Use `<!-- ... -->` in Vue templates and `#` in Python.
- Log each custom change (date, 2-line why, code markers) in `docs/fork-history.md`; longer
  per-feature notes live in `docs/custom-changes.md`.
- When syncing upstream, run `git grep -n "RP-CUSTOM"` before and after to make sure every
  custom change is preserved.
