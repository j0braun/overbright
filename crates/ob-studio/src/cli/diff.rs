//! What an edit changes, shown before it is made (`--dry-run`): the items of the mod's documents added, removed
//! or changed (a map mod's details, each prop edit, each race: the history's items), and the file's lines.

use serde_json::{Value, json};

use crate::history::doc;

/// The items of document `name` that differ between `before` and `after`, in the order of the file.
pub fn items(name: &str, before: &str, after: &str) -> Vec<Value> {
    let a = doc::items(name, before).unwrap_or_else(|| vec![(doc::WHOLE.into(), before.into())]);
    let b = doc::items(name, after).unwrap_or_else(|| vec![(doc::WHOLE.into(), after.into())]);
    let value = |t: &str| serde_json::from_str::<Value>(t).unwrap_or_else(|_| Value::String(t.into()));
    let mut out = Vec::new();
    for (k, v) in &a {
        match b.iter().find(|(l, _)| l == k) {
            None => out.push(json!({ "item": k, "change": "removed", "before": value(v) })),
            Some((_, w)) if w != v => {
                out.push(json!({ "item": k, "change": "changed", "before": value(v), "after": value(w) }))
            }
            _ => {}
        }
    }
    for (k, w) in &b {
        if !a.iter().any(|(l, _)| l == k) {
            out.push(json!({ "item": k, "change": "added", "after": value(w) }));
        }
    }
    out
}

/// The longest a line diff's middle part may be (lines before × lines after) before it is shown as a block.
const MAX_CELLS: usize = 4_000_000;

/// A line diff of `before` and `after`: unchanged lines ` `, removed `-`, added `+`, with up to `context` lines
/// around each change; skipped runs are `@@`.
pub fn lines(before: &str, after: &str, context: usize) -> Vec<String> {
    let a: Vec<&str> = before.lines().collect();
    let b: Vec<&str> = after.lines().collect();
    let pre = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let suf = a[pre..].iter().rev().zip(b[pre..].iter().rev()).take_while(|(x, y)| x == y).count();
    let (ma, mb) = (&a[pre..a.len() - suf], &b[pre..b.len() - suf]);
    let mut ops: Vec<(char, &str)> = a[..pre].iter().map(|l| (' ', *l)).collect();
    if ma.len() * mb.len() > MAX_CELLS {
        ops.extend(ma.iter().map(|l| ('-', *l)));
        ops.extend(mb.iter().map(|l| ('+', *l)));
    } else {
        // Longest common subsequence of the middle.
        let (n, m) = (ma.len(), mb.len());
        let mut t = vec![0u32; (n + 1) * (m + 1)];
        for i in (0..n).rev() {
            for j in (0..m).rev() {
                t[i * (m + 1) + j] = if ma[i] == mb[j] {
                    t[(i + 1) * (m + 1) + j + 1] + 1
                } else {
                    t[(i + 1) * (m + 1) + j].max(t[i * (m + 1) + j + 1])
                };
            }
        }
        let (mut i, mut j) = (0, 0);
        while i < n || j < m {
            if i < n && j < m && ma[i] == mb[j] {
                ops.push((' ', ma[i]));
                (i, j) = (i + 1, j + 1);
            } else if i < n && (j == m || t[(i + 1) * (m + 1) + j] >= t[i * (m + 1) + j + 1]) {
                ops.push(('-', ma[i]));
                i += 1;
            } else {
                ops.push(('+', mb[j]));
                j += 1;
            }
        }
    }
    ops.extend(a[a.len() - suf..].iter().map(|l| (' ', *l)));
    let near = |k: usize| {
        let lo = k.saturating_sub(context);
        ops[lo..(k + context + 1).min(ops.len())].iter().any(|o| o.0 != ' ')
    };
    let mut out = Vec::new();
    let mut skipped = false;
    for (k, (c, l)) in ops.iter().enumerate() {
        if *c != ' ' || near(k) {
            if skipped {
                out.push("@@".to_string());
                skipped = false;
            }
            out.push(format!("{c}{l}"));
        } else {
            skipped = true;
        }
    }
    if skipped && !out.is_empty() {
        out.push("@@".to_string());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_diff_shows_the_change_with_its_context() {
        let d = lines("a\nb\nc\nd\ne\nf\n", "a\nb\nC\nd\ne\nf\ng\n", 1);
        assert_eq!(d, ["@@", " b", "-c", "+C", " d", "@@", " f", "+g"]);
        assert!(lines("same\n", "same\n", 3).is_empty());
    }
}
