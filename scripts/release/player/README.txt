Overbright @VERSION@
====================

Reimplementation of Need for Speed: Underground 2 (2004) in Rust and wgpu.

Overbright is free, open-source software written from scratch that plays the game using the files of
your own installed copy of the original game. Its default look and behaviour follow the original; extra
features (higher resolutions, modern graphics options) are optional and can be switched off.

YOU NEED YOUR OWN COPY OF THE GAME. No game files are included: you need your own PC copy of the
game, an installed Need for Speed Underground 2 with your own SPEED2.EXE, version 1.2 (the folder
that holds SPEED2.EXE).
Today Overbright can read the game data of the European 1.2 release's SPEED2.EXE without copy
protection (4,788,224 bytes). The copy-protected retail SPEED2.EXE (SafeDisc) is recognised but
cannot be read yet; support for it is being worked on:
https://github.com/j0braun/overbright/issues/16

No texture art and no mods are included either. Texture packages (TexMod .tpf files, TexWizard
packs) and the HUD mod that you have can be added in the launcher: the main screen's Mods row opens the Mods page, which adds,
turns on and off, orders and removes them. They are kept in your own Overbright folders (below) and
belong to their authors.

Overbright is not affiliated with Electronic Arts. EA has not endorsed and does not support this
product. "Need for Speed" and "Need for Speed Underground 2" are trademarks of their owner.

@@linux
Requirements: Linux x86-64 with glibc 2.28 or newer, X11 or Wayland, ALSA or PulseAudio/PipeWire
for sound, and a Vulkan driver with the Vulkan loader: Mesa 22 or newer for AMD and Intel graphics,
or NVIDIA's own driver. Tested: Ubuntu 22.04 and 24.04, Linux Mint 22, Debian 12 and 13, Fedora 40
and later, AlmaLinux 9 (RHEL 9 family), Arch (SteamOS is Arch-based), openSUSE Leap 15.6 and
Tumbleweed. Ubuntu 20.04 and Debian 11 start it but ship an older Mesa; RHEL 8 starts it, drawing
there was not tested. If the launcher says no Vulkan driver was found, install one:
  Ubuntu, Debian, Mint:  sudo apt install libvulkan1 mesa-vulkan-drivers
  Fedora, RHEL family:   sudo dnf install vulkan-loader mesa-vulkan-drivers
  Arch, SteamOS:         sudo pacman -S vulkan-icd-loader vulkan-radeon   (Intel: vulkan-intel)
  openSUSE:              sudo zypper install libvulkan1 libvulkan_radeon libvulkan_intel

Starting
--------
1. Unpack this folder anywhere you like (for example ~/Games/overbright).
2. Run ./overbright (the launcher). It looks for the original game (common install places, Wine
   prefixes, Steam/Lutris-style folders); if it cannot find it, choose the folder that holds
   SPEED2.EXE. Then press Play.
   ./game starts the game directly, without the launcher; ./game --check-game /path/to/game checks
   a game folder and prints what it found.
3. Optional, an entry in your applications menu: copy overbright.png to
   ~/.local/share/icons/hicolor/256x256/apps/overbright.png and overbright.desktop to
   ~/.local/share/applications/, then change the Exec= line in that copy to the full path of
   the overbright program (the folder you unpacked into, followed by /overbright; the menu
   does not expand "~").

Where Overbright keeps its files
--------------------------------
  settings, logs, crash reports  ~/.local/share/overbright/   (options.txt, logs/, crashes/)
  saved games (profiles)         ~/.local/share/NFS Underground 2/<profile name>/
  cache (rebuilt when missing)   ~/.cache/overbright/
($XDG_DATA_HOME and $XDG_CACHE_HOME replace ~/.local/share and ~/.cache when they are set. With
NFSU2_DATA_DIR=<folder> everything is kept in that one folder, for a portable install.)
@@windows
Requirements: Windows 10 or 11 (64-bit), a graphics card with a Vulkan driver (Direct3D 12 is a
fallback and untested). This Windows build was tested only under Wine on Linux.

Starting
--------
1. Unpack this folder anywhere you like (for example into your Documents or next to the game).
2. Run overbright.exe (the launcher). It looks for the original game (the registry, the usual
   "EA GAMES" folders, its own folder and the folder above it); if it cannot find it, choose the
   folder that holds SPEED2.EXE. Then press Play.
   game.exe starts the game directly, without the launcher.
   Windows may warn that the program is not signed ("Windows protected your PC"); choose
   "More info", then "Run anyway".

Where Overbright keeps its files
--------------------------------
  settings, logs, crash reports  %LOCALAPPDATA%\Overbright\   (options.txt, logs\, crashes\)
  saved games (profiles)         %LOCALAPPDATA%\Overbright\NFS Underground 2\<profile name>\
  cache (rebuilt when missing)   %LOCALAPPDATA%\Overbright\cache\
Overbright never writes into the original game's folder or its save folder
(%LOCALAPPDATA%\NFS Underground 2). With NFSU2_DATA_DIR=<folder> everything is kept in that one
folder, for a portable install.
@@end

Known issues
------------
The list of known issues and differences from the original, with what you notice and any way
around it, is in the README at @SOURCE_URL@ (section "Known issues").

Reporting a problem
-------------------
Please open an issue at @SOURCE_URL@/issues and include:
- what you did and what happened (a screenshot helps),
- this version line: Overbright @VERSION@ (commit @COMMIT@),
- the newest log file from the logs folder above (the launcher's "Open log folder" button shows it)
  and, after a crash, the newest file from the crashes folder.
Log files and crash reports show your home folder as "~"; please look through them before posting.

Licence and source code
-----------------------
Overbright is free software under the GNU General Public License, version 3 or (at your option)
any later version: see @LICENSE_FILE@. You may share and change it under those terms. It comes
with NO WARRANTY.
The complete source code of this version (commit @COMMIT@) is at:
  @SOURCE_URL@
Third-party components and their licences: THIRD-PARTY-NOTICES.md (fonts, shaders, algorithms)
and @CRATES_FILE@ (the Rust libraries and runtime libraries compiled into the programs).
