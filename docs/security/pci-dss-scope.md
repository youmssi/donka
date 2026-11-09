# PCI DSS scope statement

Status: self-declared, with the refusal enforced by a test. This page is part of the trust
dossier (DNK-73) and is published as it stands; no assessor has verified it.

## The statement

Donka is a decision system, not a payment system. Neither Donka Studio nor Donka Runtime
receives, processes, stores or transmits cardholder data:

- No interface asks for a card number (a primary account number, PAN): not a form, not an
  API, not a connector Donka ships.
- A decision's input contract is refused **when it is saved** if a field names a card number
  (`pan`, `cardNumber`, `numeroCarte`, …) or carries a 13-to-19-digit Luhn-valid number in an
  example or a default (`crates/engine/src/contract.rs`, `card_fields`). The refusal names
  the field and says to remove it. A version saved from an older draft is refused the same
  way (`crates/decision`, DNK-73).

Because no cardholder data enters, Donka itself is **outside the bank's PCI DSS scope**: it
is not a payment application, stores no PAN, and needs no PAN encryption, masking or
retention control.

## The bank's part

PCI DSS still applies to the bank's own systems around Donka:

- Do not put a real card number in a contract example, a test scenario, a decision input, or
  a connector's answer: the refusal covers contracts; scenario and request data are the
  bank's to keep clean, and the decision log redacts what its contract marks `pii`.
- If the bank ever needs a decision to depend on card data, tokenise it upstream (for
  example through its payment service provider) and give Donka the token or buckets such as
  `cardPresent`, never the number. That design keeps this statement true; giving Donka the
  number would take it out of scope of this statement and into PCI DSS scope.

## Evidence

- The refusal and its tests: `cargo test -p donka-engine` (`card_fields_are_found_by_name_and_by_card_like_value`)
  and the save paths in `crates/decision`.
- This statement's status: self-declared. It becomes third-party verified only when an
  assessor says so, which the Trust page (DNK-66) will state exactly.
