---
name: implement
description: "Implement a spec or tickets with batched reads, slice-only test runs and read-only reviewers."
disable-model-invocation: true
---

Implement the work described by the user in the spec or tickets.

If the user passes a ticket reference, fetch it from the issue tracker and state its title before starting. If the reference is ambiguous, ask.

## Batch the lookups

Start from the ticket and the files, seams and ADRs it names. Read all of them in one turn, as parallel tool calls, and explore past them only where they leave a gap.

Before each step, list everything the step needs to read: files, greps, test output. Ask for all of it in one turn. A read waits for an earlier result only when it depends on that result (open the file the grep found).

## Build

Call the Skill tool with "tdd" where possible, at pre-agreed seams. Run typechecking regularly.

During the red → green loop, pass only the test files of the slice to the test runner. At the end, run the tests relevant to the change: the tests you wrote or touched, and those covering the code you changed. Do not run the full test suite; the human runs it.

A green that fails: read the whole failure output before the next edit. After two failed greens on the same test, stop the edits. Re-read the test and the code path it exercises, name the cause, then edit.

## Review and commit

Call the Skill tool with "code-review". Put the output of the last test run and of the last typecheck in both sub-agent prompts, with this line: "Review by reading. The test results in this prompt come from the main agent's run on this exact diff. The working tree belongs to the main agent: read it, and leave it as it is."

Commit your work to the current branch.

When the work came from a ticket, set the ticket's state in the issue tracker after the commit:

- **Done**: every acceptance criterion is met. Add a comment that names the commit.
- **In Review** (or the tracker's nearest state): the code is in, but part of the work stays open. Add a comment that names what is left.

When the project documents its issue tracker, use its state names and operations.

Report in five lines: what changed, which tests cover it, what stays open.
