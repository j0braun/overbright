//! MicroTalk (UTK) tables transcribed from vgmstream's
//! src/coding/libs/utkdec.c (at revision 9cb615c990f1;
//! https://github.com/vgmstream/vgmstream), with their own names.
//! Licence: ISC (vgmstream's COPYING, THIRD-PARTY-NOTICES.md), which
//! follows.
//
// Copyright (c) 2008-2025 Adam Gashlin, Fastelbja, Ronny Elfert, bnnm,
//                         Christopher Snowhill, NicknineTheEagle, bxaimc,
//                         Thealexbarney, CyberBotX, EdnessP, et al
//
// Portions Copyright (c) 2004-2008, Marko Kreen
// Portions Copyright 2001-2007  jagarl / Kazunori Ueno <jagarl@creator.club.ne.jp>
// Portions Copyright (c) 1998, Justin Frankel/Nullsoft Inc.
// Portions Copyright (C) 2006 Nullsoft, Inc.
// Portions Copyright (c) 2005-2007 Paul Hsieh
// Portions Copyright (C) 2000-2004 Leshade Entis, Entis-soft.
// Portions Public Domain originating with Sun Microsystems
//
// Permission to use, copy, modify, and distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
// ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
// ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
// OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

/// The reflection coefficients by index (`utk_rc_table`).
pub const UTK_RC_TABLE: [f32; 64] = [
    0.000000, -0.996776, -0.990327, -0.983879, -0.977431, -0.970982, -0.964534, -0.958085, -0.951637, -0.930754,
    -0.904960, -0.879167, -0.853373, -0.827579, -0.801786, -0.775992, -0.750198, -0.724405, -0.698611, -0.670635,
    -0.619048, -0.567460, -0.515873, -0.464286, -0.412698, -0.361111, -0.309524, -0.257937, -0.206349, -0.154762,
    -0.103175, -0.051587, 0.000000, 0.051587, 0.103175, 0.154762, 0.206349, 0.257937, 0.309524, 0.361111, 0.412698,
    0.464286, 0.515873, 0.567460, 0.619048, 0.670635, 0.698611, 0.724405, 0.750198, 0.775992, 0.801786, 0.827579,
    0.853373, 0.879167, 0.904960, 0.930754, 0.951637, 0.958085, 0.964534, 0.970982, 0.977431, 0.983879, 0.990327,
    0.996776,
];

/// The excitation codebooks: the normal model, then the large-pulse model (`utk_codebooks`).
pub const UTK_CODEBOOKS: [[u8; 256]; 2] = [
    [
        4, 6, 5, 9, 4, 6, 5, 13, 4, 6, 5, 10, 4, 6, 5, 17, 4, 6, 5, 9, 4, 6, 5, 14, 4, 6, 5, 10, 4, 6, 5, 21, 4, 6, 5,
        9, 4, 6, 5, 13, 4, 6, 5, 10, 4, 6, 5, 18, 4, 6, 5, 9, 4, 6, 5, 14, 4, 6, 5, 10, 4, 6, 5, 25, 4, 6, 5, 9, 4, 6,
        5, 13, 4, 6, 5, 10, 4, 6, 5, 17, 4, 6, 5, 9, 4, 6, 5, 14, 4, 6, 5, 10, 4, 6, 5, 22, 4, 6, 5, 9, 4, 6, 5, 13, 4,
        6, 5, 10, 4, 6, 5, 18, 4, 6, 5, 9, 4, 6, 5, 14, 4, 6, 5, 10, 4, 6, 5, 0, 4, 6, 5, 9, 4, 6, 5, 13, 4, 6, 5, 10,
        4, 6, 5, 17, 4, 6, 5, 9, 4, 6, 5, 14, 4, 6, 5, 10, 4, 6, 5, 21, 4, 6, 5, 9, 4, 6, 5, 13, 4, 6, 5, 10, 4, 6, 5,
        18, 4, 6, 5, 9, 4, 6, 5, 14, 4, 6, 5, 10, 4, 6, 5, 26, 4, 6, 5, 9, 4, 6, 5, 13, 4, 6, 5, 10, 4, 6, 5, 17, 4, 6,
        5, 9, 4, 6, 5, 14, 4, 6, 5, 10, 4, 6, 5, 22, 4, 6, 5, 9, 4, 6, 5, 13, 4, 6, 5, 10, 4, 6, 5, 18, 4, 6, 5, 9, 4,
        6, 5, 14, 4, 6, 5, 10, 4, 6, 5, 2,
    ],
    [
        4, 11, 7, 15, 4, 12, 8, 19, 4, 11, 7, 16, 4, 12, 8, 23, 4, 11, 7, 15, 4, 12, 8, 20, 4, 11, 7, 16, 4, 12, 8, 27,
        4, 11, 7, 15, 4, 12, 8, 19, 4, 11, 7, 16, 4, 12, 8, 24, 4, 11, 7, 15, 4, 12, 8, 20, 4, 11, 7, 16, 4, 12, 8, 1,
        4, 11, 7, 15, 4, 12, 8, 19, 4, 11, 7, 16, 4, 12, 8, 23, 4, 11, 7, 15, 4, 12, 8, 20, 4, 11, 7, 16, 4, 12, 8, 28,
        4, 11, 7, 15, 4, 12, 8, 19, 4, 11, 7, 16, 4, 12, 8, 24, 4, 11, 7, 15, 4, 12, 8, 20, 4, 11, 7, 16, 4, 12, 8, 3,
        4, 11, 7, 15, 4, 12, 8, 19, 4, 11, 7, 16, 4, 12, 8, 23, 4, 11, 7, 15, 4, 12, 8, 20, 4, 11, 7, 16, 4, 12, 8, 27,
        4, 11, 7, 15, 4, 12, 8, 19, 4, 11, 7, 16, 4, 12, 8, 24, 4, 11, 7, 15, 4, 12, 8, 20, 4, 11, 7, 16, 4, 12, 8, 1,
        4, 11, 7, 15, 4, 12, 8, 19, 4, 11, 7, 16, 4, 12, 8, 23, 4, 11, 7, 15, 4, 12, 8, 20, 4, 11, 7, 16, 4, 12, 8, 28,
        4, 11, 7, 15, 4, 12, 8, 19, 4, 11, 7, 16, 4, 12, 8, 24, 4, 11, 7, 15, 4, 12, 8, 20, 4, 11, 7, 16, 4, 12, 8, 3,
    ],
];

/// The pulse commands: (next model: 0 normal, 1 large-pulse; code size in bits; pulse value) (`utk_commands`).
pub const UTK_COMMANDS: [(u8, u8, f32); 29] = [
    (1, 8, 0.0),
    (1, 7, 0.0),
    (0, 8, 0.0),
    (0, 7, 0.0),
    (0, 2, 0.0),
    (0, 2, -1.0),
    (0, 2, 1.0),
    (0, 3, -1.0),
    (0, 3, 1.0),
    (1, 4, -2.0),
    (1, 4, 2.0),
    (1, 3, -2.0),
    (1, 3, 2.0),
    (1, 5, -3.0),
    (1, 5, 3.0),
    (1, 4, -3.0),
    (1, 4, 3.0),
    (1, 6, -4.0),
    (1, 6, 4.0),
    (1, 5, -4.0),
    (1, 5, 4.0),
    (1, 7, -5.0),
    (1, 7, 5.0),
    (1, 6, -5.0),
    (1, 6, 5.0),
    (1, 8, -6.0),
    (1, 8, 6.0),
    (1, 7, -6.0),
    (1, 7, 6.0),
];
