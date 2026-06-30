// titania-verus-binding: fixture-smoke
use vstd::prelude::*;

verus! {

proof fn formal_setup_smoke()
    ensures
        1 + 1 == 2,
{
    assert(1 + 1 == 2);
}

}

fn main() {}
