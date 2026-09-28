//! Cross-cutting code shared by the business modules (backend.md §1). Only what
//! every module needs belongs here: no business rules.

pub mod clock;
pub mod page;
