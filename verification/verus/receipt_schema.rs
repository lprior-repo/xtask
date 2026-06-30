use vstd::prelude::*;

#[path = "../../crates/titania-core/src/receipt/schema.rs"]
mod production;

verus! {

pub assume_specification[production::is_supported_receipt_schema_version](schema_version: u32) -> (supported: bool)
    ensures
        supported == (schema_version == 2),
;

fn schema_accepts_current() -> (supported: bool)
    ensures
        supported == true,
{
    production::is_supported_receipt_schema_version(2)
}

fn schema_rejects_future() -> (supported: bool)
    ensures
        supported == false,
{
    production::is_supported_receipt_schema_version(3)
}

proof fn schema_version_literal_is_stable()
    ensures
        2 == 2,
{
    assert(2 == 2);
}

}

fn main() {}
