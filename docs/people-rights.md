# Rights of the people decisions are about

Data-protection laws (Cameroon's law 2024/017, the GDPR and their neighbours) give a person the
right to know what is held about them, to have it erased when the law allows, and to contest a
decision made automatically. Studio lets a project's **owners** answer those requests from
**People's requests**, in the project's sidebar. Every search, report, anonymisation, retention
rule and contest is recorded in the project's audit log; the identifier searched never is.

## Marking the fields that identify a person

In a decision's **Input fields**, mark the fields that identify the person a decision is about
(national ID, phone, customer number) as **Identifier**. An identifier is always personal data;
other personal fields are marked **Personal data** and are removed too when records are
anonymised, but they are not searchable.

When a record arrives, Studio keeps a keyed hash (HMAC-SHA-256 under `DONKA_PERSON_INDEX_KEY`)
of each identifier it holds, beside the encrypted record. A search hashes the value typed the
same way, so nothing is decrypted to search and the index holds no identifier in clear. Case,
spaces, dashes and dots do not matter: `CM-1984-0042` and `cm 1984 0042` find the same person.

Records stored before an installation had this index are indexed by
`donka-app decision-log index-people` (see the installation guide). A record is indexed by the
fields its own release marks: records answered by a release that did not mark a field yet are
not found by it.

## Answering a request

1. **Find**: type the identifier. The project's records holding it are listed, newest first,
   across every month of the log.
2. **Access report**: a PDF in English or French of every record found: what the decision used,
   what it answered, its outcome and release, and the person's contests. When the installation
   explains decisions, each one can be explained in plain words; the record is sent to the
   explanation service without its personal and redacted fields.
3. **Anonymise**: every field the contracts mark as personal data, and the project's redacted
   fields, are removed from what each record read and answered and from its trace. The outcome,
   release and time stay for audit. It cannot be undone, and the person can no longer be found
   by their identifiers.

## Retention rules

A rule that requires keeping the project's records whole (a dispute, an anti-money-laundering
file) is documented in the project's **Settings** with what it is, its legal reference and its
last day. While a rule runs, anonymisation is refused and the rule is shown. Owners can end a
rule early; it stays listed.

## Contests

On a decision record, an owner records that the person contests it, with their reason. The
review happens in the lender's own systems; its outcome is recorded afterwards: **maintained**,
or **revised** with the new outcome, and a reason each time. A decision has one contest under
review at a time, and every contest stays in its history.

## Retention

Index rows and contests are removed with their records by the retention purge.

## API

| Method and path | Who |
|---|---|
| `POST /projects/{id}/people/search` `{ identifier }` | owners |
| `POST /projects/{id}/people/report` `{ identifier, language, explain }` → PDF | owners |
| `POST /projects/{id}/people/anonymise` `{ identifier }` | owners |
| `GET`, `POST /projects/{id}/retention-rules`; `POST …/retention-rules/{rule}/end` | members read, owners change |
| `GET`, `POST /projects/{id}/decision-log/{record}/contests` | members read, owners record |
| `POST /projects/{id}/contests/{contest}/review` | owners |

Identifiers travel in request bodies, never in an address, so they stay out of access logs.
