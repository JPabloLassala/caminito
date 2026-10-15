# Triage Labels

The skills speak in terms of five canonical triage roles. This file maps those roles to what this repo's Linear tracker uses. The labels are in Linear's "Triage" label group, so an issue has one at most.

| Role in mattpocock/skills | In our tracker                                  | Meaning                                  |
| ------------------------- | ----------------------------------------------- | ---------------------------------------- |
| `needs-triage`            | label `needs-triage`                            | Maintainer needs to evaluate this issue  |
| `needs-info`              | label `needs-info` (create it in "Triage" on first use) | Waiting on reporter for more information |
| `ready-for-agent`         | label `ready-for-agent`                         | Fully specified, ready for an AFK agent  |
| `ready-for-human`         | label `ready-for-human`                         | Requires human implementation            |
| `wontfix`                 | state Canceled                                  | Will not be actioned                     |

When a skill mentions a role (e.g. "apply the AFK-ready triage label"), use the corresponding entry from this table.

Only a `ready-for-agent` issue blocks another (see `issue-tracker.md`).
