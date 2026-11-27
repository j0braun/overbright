//! The AEMS event graphs of the `.abk` banks, run as the audio library
//! runs them (verified against SPEED2.EXE unless marked).
//!
//! A bank module's "code" (record +0x28, up to `pop esi; ret`) is x86 the
//! bank compiler emitted: a straight list of node calls on blocks of the
//! instance (`push esi; call node` with ESI the block; the file holds the
//! node id where the call target goes, the loader patches it to the node
//! table 0x817FE0[id], 0x72132C) and the moves that wire one node's
//! result into another's inputs (`mov [esi+d],eax`, `mov eax,[esi+d]`,
//! `add/sub/imul eax,[esi+d]`, the min / max pair `cmp eax,ecx; jl/jg;
//! mov eax,ecx`, `mov byte [esi+d],imm`, `add esi,imm`). Every module in
//! the install uses only those forms (checked over all 176 banks), so the
//! clone decodes them into [`Op`]s and evaluates the list; the node
//! functions are ported below. Nothing from the bank is executed.
//!
//! Instance (0x721096): the template (record +0x2C, +0x30 bytes) copied;
//! the list node at +8 runs the code with ESI = instance +0x18 every
//! library tick (0x725520 from the callback list 0x8B405C, run by 0x721DE6
//! when the integer of the accumulator 0x8183F0 changes; the tick period
//! 0x8B4054 is the configured 0x8B44D0 clamped to 10..100 ms, −1 giving
//! 20 ms, and the only writer stores −1: [`TICK_MS`]). From +0x18: with
//! record byte +0x25 the release node (0x14 bytes; its callback 0x72103A
//! sets +0x10 = 1 when the game releases its handle, 0x735550 → 0x735030
//! walks the handle's +0xC list), then record u16 +0x20 volume nodes
//! (0x1C bytes, 0x72102C: +0x18 = block[0]), with byte +0x26 the
//! parameter node (0x721046: byte +0x10 dwords of the game's block copied
//! to +0x14 on), then u16 +0x22 array nodes (0x72106C). Every module of
//! the install has exactly the release and parameter nodes.

use super::abk::Bank;
use crate::formats::bundle::{read_u16, read_u32};
use std::sync::Arc;

mod decode;
mod events;
mod instance;
/// A check aid for sound comparisons: with `NFSU2_EVENT_LOG=path`, every
/// event start, its parameter blocks (at most one row per instance per
/// 0.1 s, or on a change) and its end go to `path` as
/// `time\tname\tid\tparams` (time = the capture's seconds, see
/// [`set_log_time`]); the counterpart of the original's parmwatch.
pub mod log;
mod nodes;
mod program;
mod rng;

pub use self::decode::*;
pub use self::events::*;
pub use self::instance::*;
pub use self::program::*;
pub use self::rng::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_the_wiring_forms() {
        // push esi; mov esi,[esp+8]; push esi; call 1; mov [esi+0x10],eax;
        // add esi,0x14; mov eax,[esi]; mov ecx,[esi+4]; cmp eax,ecx;
        // jl +2; mov eax,ecx; mov [esi+8],eax; mov byte [esi],0; pop; ret.
        let c = [
            0x56, 0x8B, 0x74, 0x24, 0x08, 0x56, 0xE8, 1, 0, 0, 0, 0x89, 0x46, 0x10, 0x83, 0xC6, 0x14, 0x8B, 0x06, 0x8B,
            0x4E, 0x04, 0x3B, 0xC1, 0x7C, 0x02, 0x8B, 0xC1, 0x89, 0x46, 0x08, 0xC6, 0x46, 0x00, 0x00, 0x5E, 0xC3,
        ];
        let ops = decode(&c).unwrap();
        assert_eq!(
            ops,
            vec![
                Op::Call(1),
                Op::Store(0x10),
                Op::AddEsi(0x14),
                Op::Load(0),
                Op::LoadEcx(4),
                Op::Cmp,
                Op::Jl(8),
                Op::MovEcx,
                Op::Store(8),
                Op::StoreByte(0, 0)
            ]
        );
        assert!(decode(&[0x56, 0x8B, 0x74, 0x24, 0x08, 0x90]).is_none());
    }

    #[test]
    fn sine_quadrants() {
        assert_eq!(sine(0), 0);
        assert_eq!(sine(256), 65535);
        assert_eq!(sine(512), 0);
        assert_eq!(sine(768), -65535);
        assert_eq!(sine(128), -sine(640));
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn every_installed_module_decodes() {
        let root = crate::testkit::game_root().join("SOUND");
        let dirs = std::fs::read_dir(&root).unwrap_or_else(|e| panic!("{}: {e}", root.display()));
        let mut n = 0;
        for d in dirs.flatten() {
            for f in std::fs::read_dir(d.path()).into_iter().flatten().flatten() {
                let p = f.path();
                if !p.extension().is_some_and(|e| e.eq_ignore_ascii_case("abk")) {
                    continue;
                }
                let bank = Bank::parse(std::fs::read(&p).unwrap()).unwrap();
                let ev = Events::new(&bank);
                for (m, prog) in ev.programs.iter().enumerate() {
                    assert!(prog.is_some(), "{} module {m}", p.display());
                    n += 1;
                }
            }
        }
        assert!(n == 224, "{n}");
    }
}
