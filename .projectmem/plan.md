# KineAnchor — plan

> Editable **intent** file: ideas + plans — what we *mean to do*.
> This is NOT the event log. `events.jsonl` -> `summary.md` records what
> *happened*; this file records what we *intend*. The AI reads it at
> session start and edits it directly (like `PROJECT_MAP.md`): add ideas
> and plans, check items off, move done work down to Shipped. Plans are
> never logged as events.

## Ideas
_Loose thoughts, not yet committed to._

## Active plans
_What we're working toward now. Use `- [ ]` / `- [x]` checklists._

Full plan: `docs/ROADMAP.md`, `docs/ARCHITECTURE.md`, `docs/PROTOCOL.md`.

- [x] P0 scaffold: workspace (core, desktop bin, android-ffi), Cargo.lock tracked, SPDX, deny.toml, CI
- [x] P1 Linux layer-shell overlay + synthetic mode (spike `wayapp` vs custom SCTK glue first)
- [x] P2 phone stream over USB tether (NDJSON v1, QR pairing, watchdog fallback, fake_phone example)
- [ ] P3 Windows overlay
- [ ] P4 Android overlay + F-Droid
- [ ] P5 polish (ship tuning, controller, mDNS, tray, GNOME extension)

## Next
_Queued, but not started._

## Someday / maybe

## Shipped
_Move completed plans here so the top stays about the future._
