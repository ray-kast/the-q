use std::cmp::Ordering;

#[derive(Debug, Clone, Copy, thiserror::Error)]
#[error("No disjoint-set node found with ID {0}")]
pub struct NoNode<T>(pub T);

#[derive(Debug, Clone, Copy)]
pub struct Unioned<K> {
    pub root: K,
    pub unioned: Option<K>,
}

impl<K> Unioned<K> {
    pub fn map<J, F: Fn(K) -> J>(self, f: F) -> Unioned<J> {
        Unioned {
            root: f(self.root),
            unioned: self.unioned.map(f),
        }
    }

    #[inline]
    #[must_use]
    pub fn did_merge(self) -> bool { self.unioned.is_some() }
}

impl<K: Copy> Unioned<&K> {
    #[inline]
    #[must_use]
    pub fn copied(self) -> Unioned<K> {
        Unioned {
            root: *self.root,
            unioned: self.unioned.copied(),
        }
    }
}

pub trait ForestFind<K> {
    fn load_parent(&self, key: K) -> Option<K>;

    fn compare_exchange_parent(&self, key: K, current: K, new: K) -> Option<Result<K, K>>;
}

pub fn forest_find<K: Copy + Eq, F: ForestFind<K>>(set: &F, key: K) -> Result<K, NoNode<K>> {
    let parent = set.load_parent(key).ok_or(NoNode(key))?;

    if parent == key {
        Ok(parent)
    } else {
        let root = forest_find(set, parent).unwrap_or_else(|_| unreachable!());

        let prev = set
            .compare_exchange_parent(key, parent, root)
            .unwrap_or_else(|| unreachable!());
        assert!(prev == Ok(parent) || prev == Err(root));

        Ok(root)
    }
}

pub trait RankedUnion<K> {
    type Root;
    type Rank: Ord;

    fn find(&self, key: K) -> Result<Self::Root, NoNode<K>>;

    fn cmp_roots(&self, a: &Self::Root, b: &Self::Root) -> Ordering;

    fn rank(&self, root: &Self::Root) -> Option<Self::Rank>;

    fn merge(&mut self, root: &Self::Root, merged: &Self::Root);
}

pub fn ranked_union<K: Copy, S: RankedUnion<K>>(
    set: &mut S,
    a: K,
    b: K,
) -> Result<Unioned<S::Root>, NoNode<K>> {
    let mut a = set.find(a)?;
    let mut b = set.find(b)?;

    let cmp = set.cmp_roots(&a, &b);

    if cmp.is_eq() {
        return Ok(Unioned {
            root: a,
            unioned: None,
        });
    }

    let mut a_rank = set.rank(&a).unwrap_or_else(|| unreachable!());
    let mut b_rank = set.rank(&b).unwrap_or_else(|| unreachable!());

    match (a_rank.cmp(&b_rank), cmp) {
        (_, Ordering::Equal) => unreachable!(),
        (Ordering::Less, Ordering::Greater | Ordering::Less)
        | (Ordering::Equal, Ordering::Greater) => {
            std::mem::swap(&mut a, &mut b);
            std::mem::swap(&mut a_rank, &mut b_rank);
        },
        _ => (),
    }

    debug_assert!(a_rank
        .cmp(&b_rank)
        .then_with(|| set.cmp_roots(&b, &a))
        .is_gt());

    set.merge(&a, &b);

    Ok(Unioned {
        root: a,
        unioned: Some(b),
    })
}
