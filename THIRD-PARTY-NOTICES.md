# Third-party notices

Overbright is licensed under the GNU General Public License v3.0 or later (see LICENSE). It contains code derived from the following third-party work, each under its own license (all GPL-3-compatible), whose notices are kept below.

## AMD FidelityFX Super Resolution 1.0 (FSR 1)

Used in `src/render/fsr.wgsl` and `src/render/enh/post/fsr.rs` (EASU and RCAS passes,
ported to WGSL). Upstream: AMD GPUOpen, FidelityFX Super Resolution 1.0.

Copyright (c) 2021 Advanced Micro Devices, Inc. All rights reserved.

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in
all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

## Go fonts (Go Regular, Go Bold)

Used by the launcher (`src/launcher/fonts/Go-Regular.ttf`, `Go-Bold.ttf`,
embedded in `overbright`). Upstream: https://go.googlesource.com/image,
`font/gofont/ttfs` (fetched 2026-11-12 from github.com/golang/image, master;
SHA-256 Go-Regular.ttf 197d9f37…be3, Go-Bold.ttf c18494ba…aa40). Licence:
BSD-3-Clause (the files' own notice, `src/launcher/fonts/README-Go-fonts.txt`):

```
Copyright (c) 2016 Bigelow & Holmes Inc.. All rights reserved.

Distribution of this font is governed by the following license. If you do not
agree to this license, including the disclaimer, do not distribute or modify
this font.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are met:

	* Redistributions of source code must retain the above copyright notice,
	  this list of conditions and the following disclaimer.

	* Redistributions in binary form must reproduce the above copyright notice,
	  this list of conditions and the following disclaimer in the documentation
	  and/or other materials provided with the distribution.

	* Neither the name of Google Inc. nor the names of its contributors may be
	  used to endorse or promote products derived from this software without
	  specific prior written permission.

DISCLAIMER: THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
"AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO,
THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT OWNER OR CONTRIBUTORS BE LIABLE
FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
```

## FFmpeg (VP6 and MPEG audio tables)

Used in `src/exe_tables/third_party/ffmpeg_vp6.rs` and
`src/exe_tables/third_party/ffmpeg_mpegaudio.rs`: the VP6 decoder's tables
(libavcodec `vp56data.c`, at revision 83386a1f62c0, and
`vp6data.h`, revision 492a3b4b895c) and the MPEG-1 layer III Huffman code tables
and synthesis window of ISO/IEC 11172-3 (libavcodec `mpegaudiodec_common.c`,
revision 24914d60df9d, and `mpegaudiodsp_data.c`, revision 15a0fb58a329),
transcribed to Rust as tables only; no decoder code is taken. Upstream:
https://ffmpeg.org (https://github.com/FFmpeg/FFmpeg). Licence: GNU Lesser
General Public License, version 2.1 or (at your option) any later version.
The LGPL-2.1-or-later permits distribution under the GNU GPL version 3 (its
section 3), which is how these tables are distributed in Overbright.

```
Copyright (C) 2006  Aurelien Jacobs <aurel@gnuage.org>
copyright (c) 2002 Fabrice Bellard

FFmpeg is free software; you can redistribute it and/or
modify it under the terms of the GNU Lesser General Public
License as published by the Free Software Foundation; either
version 2.1 of the License, or (at your option) any later version.

FFmpeg is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
Lesser General Public License for more details.
```

## vgmstream (MicroTalk tables)

Used in `src/exe_tables/third_party/vgmstream_utk.rs`: the MicroTalk (UTK)
decoder's reflection coefficients, codebooks and pulse commands from
`src/coding/libs/utkdec.c` (at revision 9cb615c990f1), transcribed
to Rust as tables only. Upstream: https://github.com/vgmstream/vgmstream.
Licence: ISC (vgmstream's `COPYING`):

```
Copyright (c) 2008-2025 Adam Gashlin, Fastelbja, Ronny Elfert, bnnm,
                        Christopher Snowhill, NicknineTheEagle, bxaimc,
                        Thealexbarney, CyberBotX, EdnessP, et al

Portions Copyright (c) 2004-2008, Marko Kreen
Portions Copyright 2001-2007  jagarl / Kazunori Ueno <jagarl@creator.club.ne.jp>
Portions Copyright (c) 1998, Justin Frankel/Nullsoft Inc.
Portions Copyright (C) 2006 Nullsoft, Inc.
Portions Copyright (c) 2005-2007 Paul Hsieh
Portions Copyright (C) 2000-2004 Leshade Entis, Entis-soft.
Portions Public Domain originating with Sun Microsystems

Permission to use, copy, modify, and distribute this software for any
purpose with or without fee is hereby granted, provided that the above
copyright notice and this permission notice appear in all copies.

THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
```

## Format references (no code taken)

These mod formats are read by Overbright's own code, written from the public
sources named here; nothing of them is copied into the tree:

* TexWizard by R-033 (https://github.com/R-033/texwizard, GNU GPL v3.0; release
  1.3 and its wiki): the pack layout (`meta.json` pairs, `textures.bin`,
  `scripts/TexWizard.json`, the `TexturePacks` scan) from `TexWizard.h`. Read by
  `src/formats/texwizard.rs` (docs/texture-packs.md, "TexWizard packs").
* retex by Depth118 (https://github.com/Depth118/retex, MIT OR Apache-2.0):
  its notes on TexMod's `.tpf` container, `docs/texmod-format.md`. Read by
  `src/formats/tpf.rs` (docs/texmod-import.md).
