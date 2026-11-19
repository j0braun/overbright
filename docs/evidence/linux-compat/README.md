# Linux distributions: the release archive in containers

docs/developing/building-releases.md, "Linux: what a player needs". Made with `scripts/release/distros/run.sh` (rootless
podman, no network, no devices, the game folder read-only, Mesa's software Vulkan driver lavapipe):

- `summary-glibc235-build.txt`: the archive built on Ubuntu 22.04 (glibc 2.35), the base source.
- `summary-glibc228-build.txt`: the archive built on AlmaLinux 8 (glibc 2.28), this source.
  One line per distribution: works / FAIL, glibc, what the image as pulled lacks (`bare lacks`), the
  lavapipe version, the headless shots' times, or the first thing that failed.
- `no-vulkan.txt`: Ubuntu 24.04 without the Vulkan loader (`novk`) and with the loader but no driver
  (`nodrv`): the probe's `error=` line (the launcher's Graphics card line) and the game's window start.
- `same-frames-both-images.txt`: the same pictures and simulation state from the harness game built in
  either image.

The shots and launcher screenshots stay in the work folder (they show the game's own art).
