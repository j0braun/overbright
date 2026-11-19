# Writing the docs

How to write a page in `docs/` so that a player, a modder or a developer can use it quickly.

## Who reads it

Know which reader the page is for and write for that one first:

- **Players** want to know what they can do and where to click. Write to them as "you".
- **Modders** want to make something: the steps, the folder, the file format, the limits.
- **Developers** want to change the code: where it lives, how it is tested, what is still open.

A page for several readers starts with the player's part and ends with the technical part under a heading
such as "How it works" or "For developers".

## How to write

- Say what the reader can do first. Background, history and proofs come later, or not at all.
- Use plain words and short sentences. One idea per paragraph.
- Use the active voice: "The launcher checks the game folder", not "The game folder is checked".
- Write numbers, keys, menu paths and file names exactly: **Options → Display → Enhancements**, `F7`,
  `options.txt`, 1280 × 960.
- Keep every fact true. When you shorten, cut repetition, not facts. If a fact is only for developers, move
  it down; do not drop it.
- Mark how sure a claim is when it matters: **verified** (read in the original's code), **observed** (seen
  in the original under Wine), **inferred**, **open**. Define these once per page if the page uses them.
- Use a table when the reader compares rows (options, keys, formats). Use a list for steps.
- Link to the page that explains a term instead of explaining it again.

## Words to avoid

These words are common in machine-written text and say little. Use the plain word on the right, or cut.

| Avoid | Write |
|---|---|
| leverage, utilize | use |
| robust | reliable, or say what it survives |
| seamless, seamlessly | say what happens: "without a pause" |
| comprehensive, holistic | complete, or list what it covers |
| ensure | make sure, check, or just say what happens |
| delve into | look at, explain |
| streamlined | shorter, simpler |
| cutting-edge, state-of-the-art | new, current, or name the technique |
| in order to | to |
| it's worth noting that, note that | cut it and state the fact |
| "not X but Y" framing | say Y |
| stacked parentheses (a (b) c) | two sentences |
| em-dash asides — like this — | a comma, or a new sentence |
| unit codes, work-package codes, dates in titles | the topic in words; a "Last checked" line if the date matters |

Also leave out process talk: who asked for something, which work round found it, "this work", "the plan",
"internal evidence", private captures. Say what is true and how it was checked.

## The project's own words

Spell these out the first time a page uses them, or avoid them on player pages.

| Term | Say on a player page | Meaning |
|---|---|---|
| native, the original | "the original game" | `SPEED2.EXE`, the PC release 1.2, as it plays |
| clone | "Overbright" | this program |
| parity | "plays the same as the original" | the same results from the same start |
| deviation | "difference from the original" | a place where Overbright knowingly differs; listed in native-deviations.md |
| opt-in, extra | "optional, off until you turn it on" | a setting whose default is the original's behaviour |
| gate | "check" | a test that must pass before a change is merged |
| harness | "test tool" | the program that runs the game and the original side by side or without a window |
| hash, bin hash, hash name | "the texture's number (its hash)" | the 32-bit number the game computes from a name (`bin_hash`, 0x43DB50) |
| tick | "world update" | one 1/60 s step of the simulation |
| FE, front end | "menus" | the game's menu screens |
| NIS | "in-game scene" | the scenes the engine plays with the game's own models |

## Reference pages

Pages about how the original works (`formats.md` and its chapters, the tracing and asset-format pages) keep
their addresses, field offsets, hash values and exact terms: the reader needs them to find the code. The
rules above still apply to the sentences around them.
