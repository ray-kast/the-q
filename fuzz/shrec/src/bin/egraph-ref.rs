use fuzz_shrec::egraph::{Input, SlowGraph};

fn main() {
    afl::fuzz!(|data: Input| data.run_reference::<SlowGraph>());
}
