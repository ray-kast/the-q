use fuzz_shrec::egraph::{CongrGraph, Input, SlowGraph};

fn main() {
    afl::fuzz!(|data: Input| data.run_differential::<SlowGraph, CongrGraph>());
}
