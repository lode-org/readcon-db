# Security Policy

## Reporting a vulnerability

Email the maintainer at [rgoswami@ieee.org](mailto:rgoswami@ieee.org) with a
description, a reproducer, the affected version or tag, and the expected
impact. Do not open a public issue for security matters.

There is no bug bounty.

## Supported versions

Security fixes land on `main` and ship in the next `vX.Y.Z` tag. Older
lines receive best-effort attention; upgrade to the latest tag.

## Scope

In scope: ingest and export of untrusted CON/convel, the C ABI (`rkrdb_*`),
language bindings, LMDB corpus files, and release artifacts published from
this repository.

Out of scope: readcon-core defects that are not db-specific, and third-party
LMDB/Heed issues without a readcon-db-specific defect.
