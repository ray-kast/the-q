use std::{collections::BTreeMap, hash::Hash, num::NonZeroU8};

use strsim::{generic_damerau_levenshtein, normalized_damerau_levenshtein};

use super::{Arg, ArgType, CommandInfo, Data, Subcommand, Trie};

/// Compute the similarity between two command descriptors
#[inline]
#[must_use]
pub fn similarity(l: &CommandInfo, r: &CommandInfo) -> f64 { l.sim(r) }

#[expect(
    clippy::cast_precision_loss,
    reason = "Normalizing the score requires casting usizes to f64s"
)]
fn ngdl<T: Clone + Eq + Hash>(l: &[T], r: &[T]) -> f64 {
    let score = generic_damerau_levenshtein(l, r);
    1.0 - (score as f64) / (l.len().max(r.len()) as f64)
}

#[expect(
    clippy::cast_precision_loss,
    reason = "Dividing the sum by the sample count requires casting N to an f64"
)]
fn avg<const N: usize>(a: [f64; N]) -> f64 { a.into_iter().sum::<f64>() / N as f64 }

trait Sim {
    fn sim(&self, rhs: &Self) -> f64;
}

impl Sim for String {
    fn sim(&self, rhs: &Self) -> f64 { normalized_damerau_levenshtein(self, rhs) }
}

impl Sim for bool {
    fn sim(&self, rhs: &Self) -> f64 {
        if *self == *rhs {
            1.0
        } else {
            0.0
        }
    }
}

impl Sim for NonZeroU8 {
    fn sim(&self, rhs: &Self) -> f64 {
        if *self == *rhs {
            1.0
        } else {
            0.0
        }
    }
}

impl<T: Clone + Eq + Hash> Sim for Vec<T> {
    fn sim(&self, rhs: &Self) -> f64 { ngdl(self, rhs) }
}

#[expect(
    clippy::cast_precision_loss,
    reason = "Computing the average score requires casting len() to an f64"
)]
impl<K: Ord, V: Sim> Sim for BTreeMap<K, V> {
    fn sim(&self, rhs: &Self) -> f64 {
        self.iter()
            .filter_map(|(k, l)| {
                let r = rhs.get(k)?;

                Some(l.sim(r))
            })
            .sum::<f64>()
            / (self.len().max(rhs.len()) as f64)
    }
}

impl Sim for CommandInfo {
    fn sim(&self, rhs: &Self) -> f64 {
        let Self {
            name: l_name,
            can_dm: l_dm,
            data: l_data,
        } = self;
        let Self {
            name: r_name,
            can_dm: r_dm,
            data: r_data,
        } = rhs;

        avg([l_name.sim(r_name), l_dm.sim(r_dm), l_data.sim(r_data)])
    }
}

impl Sim for Data {
    fn sim(&self, rhs: &Self) -> f64 {
        match (self, rhs) {
            (l, r) if l == r => 1.0,
            (
                Self::Slash {
                    desc: l_desc,
                    trie: l_trie,
                },
                Self::Slash {
                    desc: r_desc,
                    trie: r_trie,
                },
            ) => avg([l_desc.sim(r_desc), l_trie.sim(r_trie)]),
            (Self::User, Self::User) | (Self::Message, Self::Message) => 1.0,
            _ => 0.0,
        }
    }
}

impl Sim for Trie {
    fn sim(&self, rhs: &Self) -> f64 {
        match (self, rhs) {
            (l, r) if l == r => 1.0,
            (
                Self::Branch {
                    height: l_height,
                    children: l_chld,
                },
                Self::Branch {
                    height: r_height,
                    children: r_chld,
                },
            ) => avg([l_height.sim(r_height), l_chld.sim(r_chld)]),
            (
                Self::Leaf {
                    args: l_args,
                    arg_order: l_order,
                },
                Self::Leaf {
                    args: r_args,
                    arg_order: r_order,
                },
            ) => avg([l_args.sim(r_args), l_order.sim(r_order)]),
            _ => 0.0,
        }
    }
}

impl Sim for Subcommand {
    fn sim(&self, rhs: &Self) -> f64 {
        let Self {
            desc: l_desc,
            node: l_node,
        } = self;
        let Self {
            desc: r_desc,
            node: r_node,
        } = rhs;

        avg([l_desc.sim(r_desc), l_node.sim(r_node)])
    }
}

impl Sim for Arg {
    fn sim(&self, rhs: &Self) -> f64 {
        let Self {
            desc: l_desc,
            required: l_req,
            ty: l_ty,
        } = self;
        let Self {
            desc: r_desc,
            required: r_req,
            ty: r_ty,
        } = rhs;

        avg([l_desc.sim(r_desc), l_req.sim(r_req), l_ty.sim(r_ty)])
    }
}

// NOTE: I don't think digging deeper than this with an O(n^2) algorithm is
//       really necessary.
impl Sim for ArgType {
    fn sim(&self, rhs: &Self) -> f64 {
        if *self == *rhs {
            1.0
        } else {
            0.0
        }
    }
}
