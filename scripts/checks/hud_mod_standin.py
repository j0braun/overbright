#!/usr/bin/env python3
"""A stand-in for the HUD mod's two downloads, made here (nothing of the mods): an extracted HUD part
folder (`assets/userstart.end` and folders of 4 x 4 DXT1 DDS images of one colour each) and a map part
ZIP (`Binary/script.end` and its track maps), with as many images, maps and calibration rows as the game's
completeness check wants (595 HUD images, 115 maps, 88 rows for 27 tracks). The same layout as
`hud_mod::tests` builds in Rust; for the Windows smoke (scripts/checks/wine_smoke.sh) and by-hand runs of
`overbright --add-hud-mod`.

  hud_mod_standin.py OUT_DIR      -> OUT_DIR/hud-part-example/, OUT_DIR/map-part-example.zip

Python 3 standard library only.
"""
import struct
import sys
import zipfile
from pathlib import Path

HUD, MAPS = 595, 115
FIELDS = ['MenuMapZoomOffsetX', 'MenuMapZoomOffsetY', 'MenuMapZoomWidth', 'TrackMapCalibrationOffsetX']
FOLDERS = ['FRONTB\\FRONTENDTEXTURES', 'INGAMECOMMON\\HUDTEXTURESGLOBAL', 'HUD_CUSTOMTEXTURES_ALL']


def dds(colour):
    """A 4 x 4 DXT1 DDS of one RGB565 colour."""
    head = bytearray(124)
    struct.pack_into('<IIII', head, 0, 124, 0x1007, 4, 4)
    struct.pack_into('<II', head, 72, 32, 4)
    head[80:84] = b'DXT1'
    return b'DDS ' + bytes(head) + struct.pack('<HHI', max(colour, 1), 0, 0)


def main():
    out = Path(sys.argv[1])
    hud = out / 'hud-part-example'
    assets = hud / 'assets'
    script = '[VERSN2]\n\nunlock_memory all\n'
    for i, f in enumerate(FOLDERS):
        script += f'bind_textures synchronize GLOBAL\\EXAMPLE{i}.BIN TPKBlocks COLL{i} "{f}"\n'
        (assets / f.replace('\\', '/')).mkdir(parents=True, exist_ok=True)
    (assets / 'userstart.end').write_text(script)
    for i in range(HUD):
        (assets / FOLDERS[i % 3].replace('\\', '/') / f'EXAMPLE_HUD_{i:03}.dds').write_bytes(dds(i * 7 + 3))
    (hud / 'ReadMe.txt').write_text('example readme\n')
    files = []
    script = '[VERSN2]\n'
    for t in range(100):
        n = 4000 + t
        script += f'replace_texture TRACKS\\TRACKMAP{n}.BIN TPKBlocks TRACKMAP{n} TRACKMAP{n} "Map\\TRACKMAP{n}.dds"\n'
        files.append((f'Binary/Map/TRACKMAP{n}.dds', dds(1000 + t)))
    for t in range(27):
        for f in FIELDS[:4 if t < 7 else 3]:
            script += f'update_collection GLOBAL\\GLOBALB.LZC Tracks {4000 + t} {f} {100 + t}\n'
    script += 'combobox "Default" "Beta" "Select map version"\n\n"Default"\n'
    for t in range(100, MAPS):
        n = 4000 + t
        script += (f'replace_texture TRACKS\\TRACKMAP{n}.BIN TPKBlocks DEFAULT TRACKMAP{n} '
                   f'"Map\\Default\\TRACKMAP{n}.dds"\n')
        files.append((f'Binary/Map/Default/TRACKMAP{n}.dds', dds(2000 + t)))
    script += '\n"Beta"\nreplace_texture TRACKS\\TRACKMAP4000.BIN TPKBlocks DEFAULT TRACKMAP4000 "Map\\Beta\\TRACKMAP4000.dds"\n'
    files += [('Binary/Map/Beta/TRACKMAP4000.dds', dds(9)), ('Binary/script.end', script.encode()),
              ('readme.txt', b'example map readme\n')]
    with zipfile.ZipFile(out / 'map-part-example.zip', 'w', zipfile.ZIP_DEFLATED) as z:
        for name, data in files:
            z.writestr(name, data)
    print(hud, out / 'map-part-example.zip')


if __name__ == '__main__':
    main()
