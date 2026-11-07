# Brief: interface for a tower-control game

Output folder: `research/out/ui/`. Topic name for `research/sources/` and
`research/work/`: `ui`.

## Why this exists

`PLAN-twr.md` already commits to an interface: a Canvas 2D map, DOM flight strips,
click-to-route taxi with a preview that is exactly the route, single-key verbs, a
typed command line with legal-next-word autocomplete, the Nothing OS Classic look
in dark, and red reserved for conflicts (§ Look and interaction, § Input). Those
choices were made from taste and one reference game. This run tests them against
evidence, and finds what they miss.

**Challenge the plan, do not decorate it.** A finding that says "the plan is right
here" needs the evidence too; a finding that says it is wrong needs the evidence
and the smallest change that fixes it.

## Read first

- `PLAN-twr.md`, all of it.
- `../md/designs.md` and `../md/design-research.md` — the house design tokens and
  earlier verified research on colour, contrast and motion. Do not redo that
  work; cite it where it applies and research only what it does not cover.

## Questions, in priority order

### 1. What do real tower systems put on screen, and why?

Real controllers' tools have decades of human-factors work behind them. Find what
is public about:

- **Electronic flight strips** (EFS / EFPS): NATS, DFS, LFV, Frequentis
  smartStrips, Indra, Thales, and what AAI uses at Delhi if public. Strip layout,
  which fields, how strips are ordered into bays (pending, clearance, taxi,
  runway, airborne), how a strip moves between bays, what gestures move it.
- **A-SMGCS / surface movement radar displays:** label (data block) contents and
  layout, how conflict alerts and runway incursion alerts are shown, colour
  conventions, leader lines, label de-cluttering.
- **Standards:** FAA Human Factors Design Standard (HF-STD-001) chapters on
  displays and alerts; EUROCONTROL HMI and alerting guidance (e.g. for A-SMGCS
  and RIMCAS); ICAO Doc 9830 (A-SMGCS manual). Extract the rules that transfer to
  a game: alert hierarchy, colour reservation, label rules, minimum sizes.

Output a table: `practice, what it is, why real systems do it (cited), adopt in
twr? (yes / adapt / no), reason`.

### 2. What do tower and ATC games do well and badly?

Study the interfaces of existing ATC and traffic-management games — for example
Tower!3D Pro, Endless ATC, BeyondATC, Airport Madness, Approach Control games,
and the reference game named in the plan — plus non-ATC games that solved the
same problem of directing many agents under time pressure (Mini Metro, Flight
Control, Overcooked-style command games, RTS command cards and control groups).
Use reviews, developer talks and postmortems (GDC Vault public talks, developer
blogs), store-page player reviews in aggregate, and community forum threads about
what frustrates players.

**Extract principles, never copy a look.** The game must not resemble any one of
them. For each source: `game, what players praise, what players complain about
(quote 1–2 representative complaints with links), the principle underneath`.

### 3. Input: how fast can a player issue a clearance?

The plan's input design rests on claims that can be checked:

- Click-to-route vs. typed commands: what is known about speed and error rates
  for each (Fitts's law, keystroke-level model (KLM) estimates, any HCI studies
  of command lines vs. direct manipulation under time pressure).
- Use KLM to estimate the time for one full departure clearance (push, taxi via
  two taxiways with a hold short, line up, take off) in each of the plan's three
  input paths. Show the operator sequence and the totals in a table.
- Target sizes: an aircraft at full zoom-out is a few pixels wide. What hit-slop
  and selection-disambiguation patterns work when targets overlap?
- Autocomplete for command entry: what makes it fast (legal-next only, ranking,
  ghost text) vs. slow.

### 4. Attention and alerting under overload

The game's Surge mode exists to overload the player. Research what keeps
overload fair:

- Situation awareness (Endsley's model) and how displays support it.
- Alert design: how many alert levels, how to avoid alarm fatigue, what
  pre-alert ("this will conflict in 30 s") does to performance.
- "Who is waiting on me" queues: the plan's `Tab` cycles to the oldest waiting
  aircraft. Is there evidence for oldest-first vs. most-urgent-first?
- Workload measures (NASA-TLX, ATWIT) used in ATC research, and whether any
  could become an in-game difficulty signal.

### 5. Reading the map

- Top-down aircraft at scale: silhouette vs. symbol vs. both by zoom level; when
  labels should appear; how real A-SMGCS displays handle zoom.
- Dark-theme map palettes that keep taxiway, runway, apron and grass distinct
  while leaving red free for conflicts; check contrast with the numbers already
  in `design-research.md`.
- Colour-vision deficiency: which of the plan's colour signals fail for
  deuteranopia and protanopia, and what shape or pattern channel backs them up.

### 6. Feedback and score

The plan prints movements, delay caused and incidents separately, never blended.
Find evidence on whether players read multi-number scores or want one, and how
games show "you caused this" for a delay or incident (replay, timeline, causal
highlight).

## What to produce

- `REPORT.md` — following the shared rules. Its "Findings that change the plan"
  section is the point of this run: each one names the plan section, the change,
  and the evidence.
- `real-systems.csv`, `games.csv` — the tables from questions 1 and 2.
- `klm.md` — the keystroke-level estimates from question 3, with the operator
  times used and their source (Card, Moran and Newell's values or a later
  revision).
- `wireframes.md` — up to three ASCII wireframes of the main screen at
  1920×1080 that apply the findings, each annotated with which finding drove
  which choice. No styling; layout and information only.

## Out of scope for this run

Sound and voice. Touch and phone layouts (out of scope in the plan). Visual
style beyond what the findings require — the house design system decides that.
