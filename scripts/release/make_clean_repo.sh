#!/bin/bash
# The clean repositories for publication (docs/releasing.md, "The clean repository"): a public repository with
# one commit of the public tree and a private companion repository with the private.list files, both local,
# no remote. Nothing is pushed, published or deleted; the source repository is only read.
#
#   make_clean_repo.sh SOURCE PUBLIC_DIR PRIVATE_DIR --name NAME --email EMAIL [--url SOURCE_URL]
#                      [--date DATE] [--message-file FILE] [--rev REV] [--work DIR] [--target-dir DIR] [--no-build]
#   make_clean_repo.sh --verify SOURCE PUBLIC_DIR PRIVATE_DIR [--rev REV] [--work DIR] [--target-dir DIR] [--no-build]
#   make_clean_repo.sh --reauthor NAME EMAIL [SOURCE_URL] REPO [REPO ...] [--date DATE]
#
# Environment instead of options: OB_AUTHOR_NAME, OB_AUTHOR_EMAIL, OB_SOURCE_URL, OB_COMMIT_DATE.
# make ends with the verify step: PASS / FAIL lines, the last one "CLEAN REPO: PASS|FAIL (...)", also in
# WORK/verify.log (WORK defaults to clean-repo-work next to PUBLIC_DIR). Exit 0 only on PASS; 2 = refused.
# --reauthor: an argument containing :// after EMAIL is the source URL; every following one is a repository.
HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
case "${1:-}" in
  --verify) shift; exec python3 -I "$HERE/clean_repo.py" verify "$@" ;;
  --reauthor) shift; exec python3 -I "$HERE/clean_repo.py" reauthor "$@" ;;
  ""|-h|--help) sed -n '2,15p' "$0" | sed 's/^# \{0,1\}//'; exit 2 ;;
  *) exec python3 -I "$HERE/clean_repo.py" make "$@" ;;
esac
