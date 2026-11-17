"""Unit tests for scan.py: python3 -m unittest discover -s scripts/release  (or python3 -m unittest scripts.release.test_scan)."""
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import scan  # noqa: E402

# Fixture strings are assembled from pieces (J) so that this file does not match the patterns it tests, and the
# owner/machine-specific families are replaced by synthetic ones injected here (the real list is families.private.txt).
HERE = Path(__file__).resolve().parent
SYNTHETIC_PRIVATE = """
[personal-name]
\\b(exampleuser|SAVEONE)\\b
[private-path]
examplecopy
[hardware]
GPU-EXAMPLE
[third-party-mod]
ExampleMod
[plan-p]
exampleuser|th[e] user
"""
SECTIONS = scan.parse_families((HERE / "families.txt").read_text())
for _k, _v in scan.parse_families(SYNTHETIC_PRIVATE).items():
    SECTIONS[_k] = SECTIONS.get(_k, []) + _v
FAM, ALLOW = scan.compile_families(SECTIONS)


def J(*parts):
    return "".join(parts)


def fams(text, path="x.md"):
    return {(f, m) for _n, f, m in scan.scan_text(path, text, FAM, ALLOW)}


def has(text, family, path="x.md"):
    return any(f == family for f, _ in fams(text, path))


class TextFamilies(unittest.TestCase):
    def test_private_paths(self):
        for s in [J("see /ho", "me/someone/x"), J("in w", "f296 and ba", "se149"), J("wi", "ne-gps69 copy"),
                  J("/tm", "p/cla", "ude-1000/foo"), J("~/Down", "loads/x"), "cp examplecopy/x"]:
            self.assertTrue(has(s, "private-path"), s)
        self.assertFalse(has(J("w", "f1 and ba", "se1 are fine, so is /usr/lib"), "private-path"))

    def test_names(self):
        for s in ["profile SAVEONE", "exampleuser's car"]:
            self.assertTrue(has(s, "personal-name"), s)
        self.assertFalse(has("exampleuserville", "personal-name"))

    def test_owner_remarks(self):
        for s in [J("th", "e user asked"), J("a user ", "report"), J("user", ": do it"), J("(user", ", 2026-11-01)"),
                  J("th", "e own", "er's choice")]:
            self.assertTrue(has(s, "owner-remark"), s)
        self.assertFalse(has("a user interface", "owner-remark"))

    def test_owner_wording(self):
        # Quoted reports and requests, dated decisions, one person's settings or machine as a test setup.
        for s in [J("Play", "er report: \"it looks off\""), J("(a pla", "yer report)"), J("Use", "r report (2026-11-04)"),
                  J("Own", "er report 2026-11-12"), J("Desi", "gn note: \"add it\""), J("at the pla", "yer's settings"),
                  J("the refer", "ence player's 4K settings"), J("4K, us", "er settings"), J("us", "er's setup"),
                  J("Enter skips (us", "er choice)"), J("us", "er decis", "ion 2026-10-31"), J("(us", "er, same day)"),
                  J("SSAO (us", "er's \"no difference\")"), J("Volumetric Fog Low (us", "er)"), J("own", "er rows 4K"),
                  J("Rate rule (own", "er):"), J("Own", "er's rule: never locked"), J("decis", "ion 2026-11-12"),
                  J("as deci", "ded 2026-11-04 for 60 fps"), J("repor", "ted by the player"), J("a test", "er's machine"),
                  J("on hi", "s desktop"), J("retook the main", "tainer"), J("the maint", "ainer's instruction")]:
            self.assertTrue(has(s, "owner-wording"), s)
        for s in ["the player's car", "a player-settings record", "the play setup at 4K", "agreed deviation",
                  "concatenate(owner)", J("the own", "er car's light"), "Wheel integration review correction (2026-11-03)",
                  "for p in orig modern user max", "the maintainer's private set", "Rachel needs her car back"]:
            self.assertFalse(has(s, "owner-wording"), s)

    def test_agent_wording(self):
        for s in [J("per th", "e lead"), J("other ag", "ents load"), J("the br", "ief says"), J("a sub", "agent")]:
            self.assertTrue(has(s, "agent-wording"), s)
        self.assertFalse(has(J("the AI ag", "ent string is empty"), "agent-wording"))

    def test_hardware_and_mods(self):
        self.assertTrue(has("a GPU-EXAMPLE desktop", "hardware"))
        self.assertTrue(has(J("an RX ", "1234 card"), "hardware"))
        self.assertTrue(has(J("k10t", "emp Tctl"), "hardware"))
        self.assertTrue(has("the ExampleMod pack", "third-party-mod"))

    def test_commit_refs(self):
        self.assertTrue(has(J("merged at com", "mit ab12cd3 of main"), "commit-ref"))
        self.assertTrue(has(J("ba", "se a1b2c3d4e5"), "commit-ref"))
        self.assertFalse(has("the base address is deadbeef", "commit-ref"))   # no digit
        self.assertFalse(has("commit message: updated", "commit-ref"))
        self.assertFalse(has("main 0x7EA2B0", "commit-ref"))

    def test_unit_codes(self):
        for s in [J("// G", "13 fix"), J("see DX", "0b"), J("# P", "14: x"), J("NX", "5 and U", "6b"), J("UX", "1a"),
                  J("D", "15 gap"), J("CK", "1")]:
            self.assertTrue(has(s, "unit-code"), s)
        for s in ["BC1 and R8 textures", "A8 format", "X11 display", "DXT5", "Menu L1 / R1", "press F3"]:
            self.assertFalse(has(s, "unit-code"), s)

    def test_hyphen_codes(self):
        for s in [J("see TRAF-", "4 here"), J("GFX-BU", "G-", "2: it was"), J("(NP-FI", "X-", "1)"), J("W", "-w3 run"),
                  J("audit game-4-0", "11"), J("audit frontend-4o-0", "12"), J("N-", "RNG order"), J("AI", "-T traced"),
                  J("TRAF-", "FIX's run"), J("N", "P-PLAY stage 2")]:
            self.assertTrue(has(s, "hyphen-code"), s)
        for s in ["UTF-8 text", "a CRC-32 table", "GPL-3.0-or-later", "MPEG-1 layer 3", "the RX-7 and the MX-5",
                  "x86-64", "SHA-256", "docs/evidence/traf-5/", "B-1 rows"]:
            self.assertFalse(has(s, "hyphen-code"), s)
        # a file under docs/native-fixes/ is named, not a remark (the unit-code family neither)
        self.assertFalse(has(J("docs/native-fixes/N-", "AUD", "2.tsv"), "hyphen-code"))
        self.assertFalse(has(J("docs/native-fixes/N-", "AUD", "2.tsv"), "unit-code"))
        self.assertTrue(has(J("// TRAF-", "3 placer"), "hyphen-code", "a.rs"))
        self.assertFalse(has(J("const TRAF_", "3: u8 = 1;"), "hyphen-code", "a.rs"))

    def test_leak_terms(self):
        with tempfile.TemporaryDirectory() as d:
            data = Path(d)
            (data / "families.txt").write_text("[personal-name]\nGENERICNAME\n")
            (data / scan.PRIVATE_FILE).write_text("[personal-name]\nexampleuser\n[leak-strings]\nsaveone_\nexampleuser\n")
            fam, _allow, _plan, n = scan.load_families_ex(data)
            self.assertEqual(1, n)                                      # leak strings are not text patterns
            self.assertNotIn("leak-strings", fam)

    def test_leak_terms_mode(self):
        # Without the private file: NOT LOADED, nothing printed, exit 3 (a check must not pass as clean).
        script = Path(__file__).resolve().parent / "scan.py"
        with tempfile.TemporaryDirectory() as d:
            tmp = Path(d)
            (tmp / "scan.py").write_text(script.read_text())
            (tmp / "families.txt").write_text("[personal-name]\nGENERICNAME\n")
            r = subprocess.run([sys.executable, "-I", str(tmp / "scan.py"), "leak-terms"], capture_output=True, text=True)
            self.assertEqual(3, r.returncode)
            self.assertEqual("", r.stdout)
            self.assertIn("NOT LOADED", r.stderr)
            (tmp / scan.PRIVATE_FILE).write_text("[leak-strings]\n# a comment\nsaveone_\nexampleuser\n")
            r = subprocess.run([sys.executable, "-I", str(tmp / "scan.py"), "leak-terms"], capture_output=True, text=True)
            self.assertEqual(0, r.returncode, r.stderr)
            self.assertEqual(["saveone_", "exampleuser"], r.stdout.split())
            self.assertIn("loaded 2", r.stderr)

    def test_rust_code_identifiers_are_masked(self):
        self.assertFalse(has(J("const C", "2: [f32; 4] = [0.0; 4];"), "unit-code", "a.rs"))
        self.assertTrue(has(J("const X: u8 = 1; // G", "13"), "unit-code", "a.rs"))
        self.assertTrue(has(J('let s = "G', '13 thing";'), "unit-code", "a.rs"))
        self.assertFalse(has(r'let s = "\u{B0}";', "unit-code", "a.rs"))

    def test_allow(self):
        line = J("  // the cursor C", "0 moves  ")
        key = scan.line_key(line)
        self.assertEqual(key, scan.line_key(line.strip()))                # leading/trailing white space does not count
        allow = [("src/*.rs", key, "unit-code", "why"), ("docs/a.md", "*", "*", "why")]
        self.assertTrue(scan.allowed(allow, "src/a.rs", line, "unit-code"))
        self.assertFalse(scan.allowed(allow, "src/a.rs", line, "hyphen-code"))
        self.assertFalse(scan.allowed(allow, "src/a.rs", J("// the cursor C", "0 stops"), "unit-code"))
        self.assertTrue(scan.allowed(allow, "docs/a.md", "anything", "private-path"))

    def test_allow_list_refuses_line_numbers(self):
        with tempfile.TemporaryDirectory() as d:
            f = Path(d) / "allow.txt"
            f.write_text("src/a.rs:12:unit-code:why\n")
            with self.assertRaises(SystemExit):
                scan.load_allow(f)
            f.write_text("src/a.rs:*:unit-code:why\nsrc/a.rs:" + scan.line_key("x") + ":*:why\n")
            self.assertEqual(2, len(scan.load_allow(f)))

    def test_private_data_loading(self):
        with tempfile.TemporaryDirectory() as d:
            data = Path(d)
            (data / "families.txt").write_text("[personal-name]\nGENERICNAME\n")
            self.assertEqual(None, scan.load_families_ex(data)[3])           # no private file: NOT LOADED
            (data / scan.PRIVATE_FILE).write_text("[personal-name]\nexampleuser\nSAVEONE\n[plan-p]\nx|y\n")
            fam, _allow, plan, n = scan.load_families_ex(data)
            self.assertEqual(2, n)
            self.assertIsNotNone(plan)
            self.assertTrue(fam["personal-name"][0].search("SAVEONE") and fam["personal-name"][0].search("GENERICNAME"))
            self.assertEqual(None, scan.load_families_ex(data, use_private=False)[3])


class Tables(unittest.TestCase):
    def test_table_with_address(self):
        body = ", ".join(str(i) for i in range(30))
        src = f"/// The table at 0x7EA2B0.\nconst T: [u8; 30] = [{body}];\n"
        self.assertEqual([(2, "T", 30, "doc")], scan.scan_tables("a.rs", src))

    def test_small_or_unaddressed(self):
        body = ", ".join(str(i) for i in range(30))
        self.assertEqual([], scan.scan_tables("a.rs", f"/// no address\nconst T: [u8; 30] = [{body}];\n"))
        self.assertEqual([], scan.scan_tables("a.rs", "/// 0x7EA2B0\nconst T: [u8; 3] = [1, 2, 3];\n"))

    def test_repeat_form_and_nested(self):
        self.assertEqual(256, scan.scan_tables("a.rs", "/// 0x401000\nconst T: [u8; 256] = [0; 256];\n")[0][2])
        rows = ", ".join("[1, 2]" for _ in range(25))
        self.assertEqual(25, scan.scan_tables("a.rs", f"/// 0x401000\nconst T: [[u8; 2]; 25] = [{rows}];\n")[0][2])

    def test_fn_local_takes_fn_doc(self):
        body = ", ".join("1" for _ in range(24))
        src = f"/// 0x459240's switch\npub fn f() {{\n    const M: [u8; 24] = [{body}];\n}}\n"
        self.assertEqual("fn-doc", scan.scan_tables("a.rs", src)[0][3])


class Binary(unittest.TestCase):
    def test_classify(self):
        self.assertEqual("personal-save", scan.classify("tests/fixtures/native_saves/a.sav"))
        self.assertEqual("game-data", scan.classify("docs/evidence/boundary-replay/handling.bin"))
        self.assertEqual("capture", scan.classify("docs/evidence/x/y.bin"))
        self.assertEqual("asset", scan.classify("src/render/a.wgsl"))
        self.assertEqual("machine-snapshot", scan.classify("tests/fixtures/overlay/root"))

    def test_refs_and_status(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            (root / "src/render").mkdir(parents=True)
            (root / "docs/evidence/x").mkdir(parents=True)
            (root / "scripts/release").mkdir(parents=True)
            (root / "src/render/a.wgsl").write_text("x")
            (root / "docs/evidence/x/c.bin").write_bytes(b"\0\1\2")
            (root / "src/a.rs").write_text(
                'fn f(){ include_str!("render/a.wgsl"); include_bytes!("../docs/evidence/x/c.bin"); }\n'
                '#[cfg(test)]\nmod t { fn g(){ include_bytes!("../docs/evidence/x/c.bin"); } }\n')
            (root / "scripts/release/private.list").write_text("docs/evidence/\n")
            (root / "scripts/release/allow-binary.txt").write_text("src/*.wgsl:asset:shaders\n")
            refs, nontext = scan.run_binary_scan(root, data=root / "scripts/release")
            by = {(r["line"], r["target"]): r for r in refs}
            self.assertEqual("ok", by[(1, "src/render/a.wgsl")]["status"])
            self.assertEqual("FAIL", by[(1, "docs/evidence/x/c.bin")]["status"])  # shipping src embeds private data
            self.assertEqual("ok", by[(3, "docs/evidence/x/c.bin")]["status"])    # inside #[cfg(test)]
            self.assertEqual([("docs/evidence/x/c.bin", "ok")], [(n["path"], n["status"]) for n in nontext])

    def test_test_only_code_and_folders(self):
        """Corrected classifications: a test-only fn inside an impl (indented #[cfg(test)]), the files of a
        `#[cfg(all(test, ...))] #[path] mod x;` / `#[cfg(test)] mod y;` are test code, not shipping source; a
        private.list folder `d/` covers a reference to the folder itself (`d`)."""
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            for p in ["src/bin/game/tests", "src/drive", "docs/evidence/x", "tests/fixtures/saves", "scripts/release"]:
                (root / p).mkdir(parents=True)
            (root / "docs/evidence/x/c.bin").write_bytes(b"\0\1\2")
            (root / "tests/fixtures/saves/a.sav").write_bytes(b"\0")
            (root / "src/ground.rs").write_text(
                'impl G {\n    #[cfg(test)]\n    fn h(&self) {\n        include_bytes!("../docs/evidence/x/c.bin");\n    }\n'
                '    fn ship(&self) {\n        include_bytes!("../docs/evidence/x/c.bin");\n    }\n}\n'
                '#[cfg(test)]\nmod t {\n    #[test]\n    fn f() { let s = "{"; std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/saves"); }\n}\n')
            (root / "src/bin/game.rs").write_text('#[cfg(all(test, feature = "dev-harness"))]\n#[path = "game/tests/mod.rs"]\nmod tests;\n')
            (root / "src/bin/game/tests/mod.rs").write_text("mod a;\n")
            (root / "src/bin/game/tests/a.rs").write_text('fn f() { include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/x/c.bin")); }\n')
            (root / "src/drive/mod.rs").write_text("#[cfg(test)]\nmod replay;\n")
            (root / "src/drive/replay.rs").write_text('fn f() { include_bytes!("../../docs/evidence/x/c.bin"); }\n')
            (root / "scripts/release/private.list").write_text("docs/evidence/\ntests/fixtures/saves/\n")
            refs, _ = scan.run_binary_scan(root, data=root / "scripts/release")
            by = {(r["src"], r["line"]): r for r in refs}
            self.assertEqual(("test-only", "ok"), ("test-only" if by[("src/ground.rs", 4)]["test"] else "shipping", by[("src/ground.rs", 4)]["status"]))
            self.assertEqual("FAIL", by[("src/ground.rs", 7)]["status"])     # the shipping fn after it still fails
            self.assertEqual("ok", by[("src/ground.rs", 13)]["status"])      # the folder itself is under saves/
            self.assertEqual("tests/fixtures/saves", by[("src/ground.rs", 13)]["target"])
            self.assertTrue(by[("src/bin/game/tests/a.rs", 1)]["test"])
            self.assertEqual("ok", by[("src/bin/game/tests/a.rs", 1)]["status"])
            self.assertTrue(by[("src/drive/replay.rs", 1)]["test"])


class Cli(unittest.TestCase):
    def test_walk_fallback_and_allow_list(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            data = root / "scripts/release"
            data.mkdir(parents=True)
            (data / "families.txt").write_text((HERE / "families.txt").read_text())
            for n in ["private.list", "allow.txt"]:
                (data / n).write_text("")
            (root / "a.md").write_text(J("see /ho", "me/someone/x\n"))      # not a git repo: the tree is walked
            hits = [h for h in scan.run_text_scan(root, "tree", data=data) if h[0] == "a.md"]
            self.assertEqual([("a.md", 1, "private-path", False)], [(h[0], h[1], h[2], h[4]) for h in hits])
            # The walk leaves out build output at the top only, not a source folder named like it.
            for sub in ["targets", "target"]:
                (root / "src" / sub).mkdir(parents=True)
                (root / "src" / sub / "b.md").write_text(J("see /ho", "me/someone/x\n"))
            (root / "target").mkdir()
            (root / "target" / "c.md").write_text(J("see /ho", "me/someone/x\n"))
            walked = {h[0] for h in scan.run_text_scan(root, "tree", data=data)}
            self.assertEqual({"a.md", "src/targets/b.md", "src/target/b.md"}, walked)
            for f in ["src/targets/b.md", "src/target/b.md", "target/c.md"]:
                (root / f).unlink()
            hit_line = J("see /ho", "me/someone/x")
            (data / "allow.txt").write_text(f"a.md:{scan.line_key(hit_line)}:private-path:test\n")
            hits = [h for h in scan.run_text_scan(root, "tree", data=data) if h[0] == "a.md"]
            self.assertTrue(hits and all(h[4] for h in hits))
            # Lines inserted above the allowed one: still allowed, now on line 3. Another line with the same
            # token is not.
            (root / "a.md").write_text("intro\nmore\n" + hit_line + "\n" + J("and /ho", "me/someone/y\n"))
            hits = [(h[1], h[4]) for h in scan.run_text_scan(root, "tree", data=data) if h[0] == "a.md"]
            self.assertEqual([(3, True), (4, False)], hits)
            import contextlib, io
            out = io.StringIO()
            with contextlib.redirect_stdout(out):
                self.assertEqual(0, scan.main(["allow-key", "a.md", "3", "--root", str(root)]))
            self.assertTrue(out.getvalue().startswith(f"a.md:{scan.line_key(hit_line)}:"))
            (data / "private.list").write_text("a.md\n")
            self.assertEqual([], [h for h in scan.run_text_scan(root, "tree", data=data) if h[0] == "a.md"])


class UnusedAllow(unittest.TestCase):
    """unused-allow: entries of the three allow lists that match nothing are listed, used ones are not."""

    def test_unused_entries(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d) / "repo"
            data = root / "scripts/release"
            data.mkdir(parents=True)
            (data / "families.txt").write_text((HERE / "families.txt").read_text())
            hit = J("see /ho", "me/someone/x")
            (root / "a.md").write_text(hit + "\n")
            (root / "p.md").write_text(hit + "\n")
            (root / "pic.png").write_bytes(b"\x89PNG\0\0")
            (data / "private.list").write_text("p.md\n")
            (data / "allow.txt").write_text(
                "# comment\n"
                f"a.md:{scan.line_key(hit)}:private-path:used\n"
                "a.md:*:unit-code:wrong family\n"
                "gone.md:*:*:file gone\n"
                "p.md:*:private-path:private file only\n")
            (data / "allow-binary.txt").write_text("pic.png:asset:used\nold/*.png:asset:nothing there\n")
            arch = Path(d) / "arch"
            arch.mkdir()
            (arch / "README.txt").write_text("hello " + hit + "\n")
            (data / "allow-archive.txt").write_text("README*:private-path:someone:used\ngame*:*:anything:no game file\n")
            fam = scan.load_families_ex(data, use_private=False)[:2]
            entries, unused, private_only = scan.unused_text_allow(root, data=data, families=fam)
            self.assertEqual(4, len(entries))
            self.assertEqual([3, 4], [no for no, _ in unused])
            self.assertEqual([5], [no for no, _ in private_only])
            entries, unused = scan.unused_binary_allow(root, data=data)
            self.assertEqual([(2, "old/*.png:asset:nothing there")], unused)
            entries, unused = scan.unused_archive_allow([arch], data=data, families=fam[0])
            self.assertEqual([2], [no for no, _ in unused])


class Archive(unittest.TestCase):
    """scan.py archive (package.sh): private strings in text and binaries, embedded bytes of a reference file."""

    def test_text_binary_and_game_data(self):
        import random
        with tempfile.TemporaryDirectory() as d:
            root = Path(d) / "a"
            root.mkdir()
            (root / "README.txt").write_text("Overbright\nsee " + J("/ho", "me/x/secret") + "\n")
            (root / "LICENSE").write_text("clean text\n")
            rnd = random.Random(7)
            game = bytes(rnd.randrange(256) for _ in range(4096))
            exe = b"\0" * 100 + J("C:", "\\Users\\someone\\x").encode("utf-16-le") + b"\0" * 9 + game[1000:1200] + b"\0" * 50
            (root / "game.exe").write_bytes(exe)
            (Path(d) / "ref.bin").write_bytes(game)
            hits = scan.run_archive_text(root, families=FAM)
            got = {(h[0], h[2]) for h in hits}
            self.assertIn(("README.txt", "private-path"), got)
            self.assertIn(("game.exe", "private-path"), got)
            self.assertFalse(any(h[0] == "LICENSE" for h in hits))
            runs = scan.game_data_runs([root / "game.exe"], [Path(d) / "ref.bin"])
            self.assertEqual(1, len(runs))
            self.assertEqual((200, 1000), (runs[0][4], runs[0][3]))
            (root / "game.exe").write_bytes(exe.replace(game[1000:1200], game[1000:1040]))  # 40 bytes: below the limit
            self.assertEqual([], scan.game_data_runs([root / "game.exe"], [Path(d) / "ref.bin"]))


if __name__ == "__main__":
    unittest.main()
