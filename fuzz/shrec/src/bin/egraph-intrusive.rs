use fuzz_shrec::egraph::{Input, IntrusiveGraph, SlowGraph};

fn main() {
    afl::fuzz!(|data: Input| data.run_differential::<SlowGraph, IntrusiveGraph>());
}
