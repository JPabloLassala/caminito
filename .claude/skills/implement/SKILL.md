---
name: implement
description: "Implement a piece of work based on a spec or set of tickets."
disable-model-invocation: true
---

Implement the work described by the user in the spec or tickets.

Use /tdd where possible, at pre-agreed seams.

Run typechecking regularly, and the tests relevant to the change regularly: the tests you wrote or touched, and those covering the code you changed. Don't run the full test suite; the human runs it.

Once done, use /code-review to review the work.

Commit your work to the current branch.
