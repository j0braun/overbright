//! A package's tree index (parents, queue order, guids), cached per thread.

use crate::frontend::feng::Package;

/// What [`layout`] needs of a package's tree, which only its objects'
/// guids and parents decide: each object's parent (its index; not
/// itself) and its place in the traversal's queue order.
pub(crate) struct TreeIndex {
    pub(crate) parents: Vec<Option<usize>>,
    pub(crate) order: Vec<usize>,
    /// Object index by guid (the last of equal guids, as `collect` keeps).
    pub(crate) by_guid: crate::fxhash::FxHashMap<u32, usize>,
}

/// The package's [`TreeIndex`], from a small cache (the HUD and menus lay
/// out the same packages every frame). An entry is reused only when the
/// package's guids and parents are still those it was built from.
pub(crate) fn tree_index(package: &Package) -> std::rc::Rc<TreeIndex> {
    type Entry = (Vec<(u32, Option<u32>)>, std::rc::Rc<TreeIndex>);
    thread_local! {
        static CACHE: std::cell::RefCell<Vec<Entry>> = const { std::cell::RefCell::new(Vec::new()) };
    }
    let same = |tree: &[(u32, Option<u32>)]| {
        tree.len() == package.objects.len()
            && tree.iter().zip(&package.objects).all(|(t, o)| t.0 == o.guid && t.1 == o.parent)
    };
    if let Some(hit) = CACHE.with_borrow(|c| c.iter().find(|e| same(&e.0)).map(|e| e.1.clone())) {
        return hit;
    }
    let by_guid: crate::fxhash::FxHashMap<u32, usize> =
        package.objects.iter().enumerate().map(|(i, o)| (o.guid, i)).collect();
    let parents: Vec<Option<usize>> = package
        .objects
        .iter()
        .enumerate()
        .map(|(i, o)| o.parent.and_then(|p| by_guid.get(&p)).copied().filter(|&p| p != i))
        .collect();
    // The traversal's queue order (0x536720 → 0x52C5E0): the top-level
    // objects, a group's children right after it, depth first (each
    // list in the package's order, **inferred**).
    let mut order = vec![usize::MAX; package.objects.len()];
    {
        let mut children: crate::fxhash::FxHashMap<usize, Vec<usize>> = Default::default();
        let mut roots = Vec::new();
        for (i, o) in package.objects.iter().enumerate() {
            match o.parent.and_then(|p| by_guid.get(&p)).copied().filter(|&p| p != i) {
                Some(g) => children.entry(g).or_default().push(i),
                None => roots.push(i),
            }
        }
        let mut next = 0;
        let mut stack: Vec<usize> = roots.into_iter().rev().collect();
        while let Some(i) = stack.pop() {
            if order[i] != usize::MAX {
                continue;
            }
            order[i] = next;
            next += 1;
            if let Some(c) = children.get(&i) {
                stack.extend(c.iter().rev());
            }
        }
        for o in order.iter_mut().filter(|o| **o == usize::MAX) {
            *o = next;
            next += 1;
        }
    }
    let index = std::rc::Rc::new(TreeIndex { parents, order, by_guid });
    CACHE.with_borrow_mut(|c| {
        if c.len() >= 32 {
            c.remove(0);
        }
        c.push((package.objects.iter().map(|o| (o.guid, o.parent)).collect(), index.clone()));
    });
    index
}
