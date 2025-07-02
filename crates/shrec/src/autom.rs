use std::{collections::BTreeSet, sync::Arc};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ClosedAccept<T, E> {
    Node(T),
    Edge(E),
}

pub trait Accept {
    type Token;

    fn as_token(&self) -> Option<&Self::Token>;
}

pub trait IntoAccept: Accept {
    fn into_token(self) -> Option<Self::Token>;
}

// TODO: never type wen eta
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NoToken {}

impl Accept for () {
    type Token = NoToken;

    #[inline]
    fn as_token(&self) -> Option<&Self::Token> { None }
}

impl IntoAccept for () {
    #[inline]
    fn into_token(self) -> Option<Self::Token> { None }
}

impl Accept for bool {
    type Token = ();

    #[inline]
    fn as_token(&self) -> Option<&Self::Token> { self.then_some(&()) }
}

impl IntoAccept for bool {
    #[inline]
    fn into_token(self) -> Option<Self::Token> { self.then_some(()) }
}

impl<T> Accept for Option<T> {
    type Token = T;

    #[inline]
    fn as_token(&self) -> Option<&Self::Token> { self.as_ref() }
}

impl<T> IntoAccept for Option<T> {
    #[inline]
    fn into_token(self) -> Option<Self::Token> { self }
}

impl<T> Accept for BTreeSet<T> {
    type Token = BTreeSet<T>;

    #[inline]
    fn as_token(&self) -> Option<&Self::Token> { (!self.is_empty()).then_some(self) }
}

impl<T> IntoAccept for BTreeSet<T> {
    #[inline]
    fn into_token(self) -> Option<Self::Token> { (!self.is_empty()).then_some(self) }
}

impl<T: Accept> Accept for Arc<T> {
    type Token = T::Token;

    #[inline]
    fn as_token(&self) -> Option<&Self::Token> { T::as_token(self) }
}

impl<T: Clone + IntoAccept> IntoAccept for Arc<T> {
    #[inline]
    fn into_token(self) -> Option<Self::Token> { T::clone(&*self).into_token() }
}

impl<T: Accept> Accept for &T {
    type Token = T::Token;

    #[inline]
    fn as_token(&self) -> Option<&Self::Token> { T::as_token(self) }
}

impl<T: Clone + IntoAccept> IntoAccept for &T {
    #[inline]
    fn into_token(self) -> Option<Self::Token> { T::clone(self).into_token() }
}
