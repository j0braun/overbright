#!/bin/bash
# A pull request to the public repository from a commit of the development repository (docs/releasing.md,
# "Changes after the first release"): the commit's public tree, selected as make_clean_repo.sh selects it
# (public_tree.py: private.list applied, the private workspace members left out of Cargo.toml and Cargo.lock),
# copied over a new branch of a fresh clone of the public repository, checked, committed with a sign-off,
# pushed and opened as a pull request. Nothing is merged, nothing is pushed to the base branch, and no history
# of the development repository leaves it: the branch gets one new commit on top of the public base.
#
#   sync_pr.sh --branch NAME --title TITLE --body-file FILE [--rev REV] [--source DIR] [--repo OWNER/NAME]
#              [--base BRANCH] [--message-file FILE] [--work DIR] [--no-push]
#
#   --rev           the development commit (default HEAD of --source); its work tree state is not used
#   --source        the development repository (default: the one holding this script)
#   --repo          the public repository on GitHub (default j0braun/overbright)
#   --base          the branch the pull request goes into (default main)
#   --message-file  the commit message's body (default: none; the subject is TITLE)
#   --work          where the export and the clone go (default: sync-pr-work next to --source; never inside it)
#   --no-push       stop after the commit (the clone stays in WORK for a look)
#
# The checks before the commit, each printed as ok / FAIL (any FAIL stops the script, exit 1):
#   - none of the branch's files is on private.list;
#   - the release scanner on the branch's tree: scan.py text (scopes tree, public, all) and scan.py binary at 0
#     (needs families.private.txt, so run it from the full development tree);
#   - no file name and no file content holds the development folder's old name (nfsu2 - or _ clone, any case);
#   - the commit message passes the same families (the co-author credit line of clean_repo.py excepted);
#   - `cargo metadata` without --locked leaves the branch's tree unchanged (the lock an outside build resolves).
# Identity: author and committer j0braun <52372377+j0braun@users.noreply.github.com>, `commit -s`, the
# co-author credit line clean_repo.py's COMMIT_CREDIT_LINES accepts. Push and pull request through gh's
# credentials (`gh auth login` first).
set -euo pipefail
HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)

die() { echo "sync_pr: $*" >&2; exit 2; }
fail() { echo "FAIL  $*"; echo "SYNC PR: FAIL"; exit 1; }
ok() { echo "ok    $*"; }

REV=HEAD SOURCE= REPO=j0braun/overbright BASE=main BRANCH= TITLE= BODY= MESSAGE= WORK= PUSH=1
while [ $# -gt 0 ]; do
  case "$1" in
    --rev) REV=$2; shift 2 ;;
    --source) SOURCE=$2; shift 2 ;;
    --repo) REPO=$2; shift 2 ;;
    --base) BASE=$2; shift 2 ;;
    --branch) BRANCH=$2; shift 2 ;;
    --title) TITLE=$2; shift 2 ;;
    --body-file) BODY=$2; shift 2 ;;
    --message-file) MESSAGE=$2; shift 2 ;;
    --work) WORK=$2; shift 2 ;;
    --no-push) PUSH=0; shift ;;
    -h|--help) sed -n '2,30p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) die "unknown argument: $1 (--help)" ;;
  esac
done
[ -n "$BRANCH" ] && [ -n "$TITLE" ] && [ -n "$BODY" ] || die "--branch, --title and --body-file are needed (--help)"
[ -f "$BODY" ] || die "$BODY: no such file"
[ -z "$MESSAGE" ] || [ -f "$MESSAGE" ] || die "$MESSAGE: no such file"
case "$BRANCH" in "$BASE"|main|master|HEAD|-*|*..*|*' '*) die "branch name $BRANCH refused (never the base branch)" ;; esac
command -v gh >/dev/null || die "gh is needed (GitHub's command line)"
BODY=$(realpath "$BODY")
[ -z "$MESSAGE" ] || MESSAGE=$(realpath "$MESSAGE")

SOURCE=$(git -C "${SOURCE:-$HERE}" rev-parse --show-toplevel)
COMMIT=$(git -C "$SOURCE" rev-parse --verify "$REV^{commit}") || die "$REV: not a commit of $SOURCE"
WORK=$(realpath -m "${WORK:-$(dirname "$SOURCE")/sync-pr-work}")
case "$WORK/" in "$SOURCE"/*) die "$WORK: inside the development repository" ;; esac
RUN="$WORK/$BRANCH-$(date +%Y%m%d-%H%M%S)"
mkdir -p "$RUN"
TREE="$RUN/tree" PUB="$RUN/public"
echo "sync_pr: $SOURCE at ${COMMIT:0:12} -> $REPO branch $BRANCH (base $BASE); work $RUN"

# The public tree of the commit, as make_clean_repo.sh selects it.
python3 -I "$HERE/public_tree.py" "$TREE" --rev "$COMMIT" --root "$SOURCE"

# A fresh clone of the public repository, the new branch from its base.
git clone --quiet --no-tags --branch "$BASE" "https://github.com/$REPO.git" "$PUB"
if [ -n "$(git -C "$PUB" ls-remote --heads origin "$BRANCH")" ]; then
  die "branch $BRANCH exists in $REPO already: pick another name"
fi
git -C "$PUB" switch --quiet -c "$BRANCH"
# The clone's files replaced by the tree's (deleted ones go too), modes kept.
rsync -a --delete --exclude /.git "$TREE/" "$PUB/"
git -C "$PUB" add -A
if git -C "$PUB" diff --cached --quiet; then
  echo "sync_pr: the public tree of ${COMMIT:0:12} equals $BASE: nothing to do"
  exit 0
fi
git -C "$PUB" diff --cached --stat | tail -1

# The checks.
CREDIT=$(python3 -I -c 'import sys; sys.path.insert(0, sys.argv[1]); import clean_repo; print(*sorted(clean_repo.COMMIT_CREDIT_LINES), sep="\n")' "$HERE")
[ "$(printf '%s\n' "$CREDIT" | wc -l)" = 1 ] || fail "clean_repo.py's COMMIT_CREDIT_LINES: want one line"
MSG="$RUN/commit-message.txt"
{
  printf '%s\n' "$TITLE"
  if [ -n "$MESSAGE" ]; then printf '\n'; cat "$MESSAGE"; fi
  printf '\n%s\n' "$CREDIT"
} > "$MSG"
python3 -I - "$HERE" "$SOURCE" "$COMMIT" "$PUB" "$RUN" "$MSG" <<'PY' || fail "the checks (above)"
import re, subprocess, sys
from pathlib import Path
here, source, commit, pub, run, msg = sys.argv[1:]
sys.path.insert(0, here)
import clean_repo, public_tree, scan
pub, run = Path(pub), Path(run)

class R(clean_repo.Report):
    def line(self, good, text):
        print(f"{'ok  ' if good else 'FAIL'}  {text}", flush=True)
        self.lines.append(("PASS" if good else "FAIL") + "  " + text)
        return good

r = R(run / "checks.log")
files = subprocess.run(["git", "-C", str(pub), "ls-files", "-z"], capture_output=True, check=True).stdout.decode().split("\0")
files = [f for f in files if f]
private = public_tree.private_patterns(subprocess.run(["git", "-C", source, "show", f"{commit}:{public_tree.PRIVATE_LIST}"],
                                                      capture_output=True, check=True).stdout.decode())
on_list = [f for f in files if scan.under(f, private)]
for f in on_list[:10]:
    print(f"      {f}")
r.line(not on_list, f"none of the branch's {len(files)} files is on private.list ({len(on_list)} are)")
clean_repo.scanner_check(r, pub, run)
old = re.compile(rb"nfsu2[-_]clone", re.I)
hits = [f"{f} (name)" for f in files if old.search(f.encode())]
for f in files:
    p = pub / f
    if p.is_file() and not p.is_symlink() and old.search(p.read_bytes()):
        hits.append(f)
for h in hits[:10]:
    print(f"      {h}")
r.line(not hits, f"the development folder's old name in no file name and no file content ({len(hits)} files)")
fam, _ = clean_repo.private_regexes()
text = "\n".join(l for l in Path(msg).read_text().split("\n") if l not in clean_repo.COMMIT_CREDIT_LINES)
bad = [f"{n}: {m.group(0)}" for n, regs in fam.items() for rg in regs for m in rg.finditer(text)]
bad += [f"old name: {m.group(0)}" for m in re.finditer(r"nfsu2[-_]clone", text, re.I)]
for b in bad[:10]:
    print(f"      {b}")
r.line(not bad, f"the commit message holds no private pattern ({len(bad)} hits)")
# A plain `cargo metadata` (no --locked: an outside developer's first build) must leave the branch's files as
# staged (clean_repo.lock_check compares against a commit; here the changes are staged, not committed yet).
log = run / "metadata.log"
with open(log, "wb") as f:
    rc = subprocess.run(["cargo", "metadata", "--format-version", "1"], cwd=pub,
                        env=clean_repo.build_env(run, run / "target"), stdout=subprocess.DEVNULL, stderr=f).returncode
changed = subprocess.run(["git", "-C", str(pub), "status", "--porcelain", "--untracked-files=all"],
                         capture_output=True, check=True, text=True).stdout.splitlines()
changed = [c for c in changed if c[1] != " "]
r.line(rc == 0 and not changed, f"cargo metadata without --locked leaves the branch's files as they are: exit {rc}"
                                + (f", changed: {changed[:5]}" if changed else ""))
sys.exit(1 if any(l.startswith("FAIL") for l in r.lines) else 0)
PY

# The commit: one, on top of the public base, signed off.
git -C "$PUB" -c user.name=j0braun -c user.email=52372377+j0braun@users.noreply.github.com \
  commit --quiet -s -F "$MSG"
NEW=$(git -C "$PUB" rev-parse HEAD)
[ "$(git -C "$PUB" rev-list --count "origin/$BASE..HEAD")" = 1 ] || fail "the branch is not one commit on $BASE"
ok "commit ${NEW:0:12} on $BASE ($(git -C "$PUB" rev-parse --short "origin/$BASE")): $TITLE"
if [ "$PUSH" = 0 ]; then
  echo "SYNC PR: ok, not pushed (--no-push); the clone: $PUB"
  exit 0
fi
git -C "$PUB" -c credential.helper= -c 'credential.helper=!gh auth git-credential' push --quiet origin "refs/heads/$BRANCH:refs/heads/$BRANCH"
ok "pushed $BRANCH"
URL=$(cd "$PUB" && gh pr create --repo "$REPO" --base "$BASE" --head "$BRANCH" --title "$TITLE" --body-file "$BODY")
ok "pull request: $URL"
echo "SYNC PR: ok $URL"
