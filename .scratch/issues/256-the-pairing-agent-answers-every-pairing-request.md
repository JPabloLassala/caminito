# The pairing agent answers every pairing request

Epic: bluetooth
Type: implementation
Status: ready-for-agent
Blocked by: 255
Spec: [Bluetooth](../bluetooth/spec.md)

## What to build

"The pairing agent" in [the spec](../bluetooth/spec.md).

- At start-up, `caminito-bluetooth` registers an `org.bluez.Agent1` with `AgentManager1` and asks to be the default agent.
- **Dialogs** (GTK4):
  - Entering a PIN or passkey (`RequestPinCode`, `RequestPasskey`).
  - Showing one to type on the device (`DisplayPinCode`, `DisplayPasskey`).
  - Confirming a number (`RequestConfirmation`).
  - Allowing a pairing that the device started (`RequestAuthorization`).
- `AuthorizeService` is allowed silently for a paired, trusted device and asks otherwise.
- `Cancel` closes the open dialog. Declining replies with `org.bluez.Error.Rejected`.
- A device paired through the agent is marked `Trusted`.

## Acceptance criteria

- [ ] Tests against dbusmock: each request reaches the agent and is answered or rejected, and the dialog is driven through a seam that the tests replace.
- [ ] `AuthorizeService` for a trusted device is allowed without a dialog.
- [ ] A paired device ends up `Trusted`.
- [ ] A check script for the developer pairs a real keyboard and a real phone.
