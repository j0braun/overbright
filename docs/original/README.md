# How the original works

What is known about `SPEED2.EXE` (the PC release 1.2) and its files, with addresses. The main reference is
the address map, [formats.md](../formats.md), split into [chapters](../formats/); names for addresses are in
[symbols.tsv](../symbols.tsv). The pages here go deeper into single topics.

| Page | What it covers |
|---|---|
| [Uncapped FPS](uncapped-fps.md) | What the original does above 60 Hz, and how Overbright plays the same at any frame rate. |
| [Random numbers](rng.md) | The original's generators, where each one is drawn, and in which order. |
| [The engine's tables](exe-tables.md) | The engine's built-in tables and how Overbright carries them without reading `SPEED2.EXE`. |
| [The start's and the menus' waits](start-and-menus.md) | What a player waits for between the start and driving. |
| [Loading screen](loading-image.md) | The start's PC_Loading screen and the HD loading and title pictures. |
| [The game's exit](exit.md) | What happens when the window closes or the game quits. |
| [Focus loss](focus-loss.md) | What the game does in the background: it pauses (traced and observed), and the In the Background row. |
| [Save points](save-points.md) | Save points, save prompts and Autosave. |
| [Race ending selection](race-ending-selection.md) | Which ending scene plays after a race. |
| [Outrun](outrun.md) | The career's random outrun racers, end to end. |
| [NIS mover](nis-mover.md) | Race-start scene cars driven by physics (mode 5). |
| [NIS car-audio patterns](nis-audio-patterns.md) | The authored car-sound commands in the in-game scenes. |
| [Engine volume chain](engine-volume-chain.md) | How the player car's engine volume is computed, step by step. |
| [Audio levels](audio-levels.md) | The option volumes, the master volume law and the level balance. |
| [Menu music](menu-music.md) | Which EA Trax song plays when. |

## Coverage maps

[coverage/](coverage/README.md): for a subsystem, every function of the original that decides what the
player sees, and whether Overbright has it ported, simplified, guessed or missing:
[traffic AI](coverage/traffic-ai.md) and [racer AI](coverage/racer-ai.md), each with a ranked list of
the gaps.

## Tracing rounds

[tracing/](tracing/README.md): what an editor or a mod needs from the original, topic by topic.
