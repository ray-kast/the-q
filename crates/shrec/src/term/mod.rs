use std::hash::Hash;

pub trait TermContext {
    type Term: Term;
    type NoArgError: std::error::Error;

    fn term_from_arg(
        &self,
        arg: <Self::Term as Term>::Arg<'_>,
    ) -> Result<&Self::Term, Self::NoArgError>;
}

pub trait Term {
    type Context: TermContext;
    type Operation: Eq + Hash;
    type Arg<'a>: Copy
    where Self: 'a;

    fn arity(&self) -> usize;

    fn arg(&self, idx: usize) -> Option<Self::Arg<'_>>;
}

pub trait TermArg<'a, C: TermContext>: Sized
where C::Term: Term<Arg<'a> = Self> + 'a
{
    fn term(self, ctx: &C) -> Result<&C::Term, C::NoArgError> { ctx.term_from_arg(self) }
}
