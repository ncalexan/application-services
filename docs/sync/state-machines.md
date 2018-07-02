---
title: State Machines
---

## FxA

Authenticating to Sync with FxA is a multi-step process that requires coordination between the client, [FxA auth server](https://github.com/mozilla/fxa-auth-server/blob/ed9c6a0962f0325633ca8d8920fadcee4c5e7c77/docs/api.md), and [token server](https://mozilla-services.readthedocs.io/en/latest/token/apis.html).

On Firefox Desktop, the logic for signing in to FxA, signing a BrowserID assertion, and exchanging the assertion for a token is spread across [`FxAccounts.jsm`](https://searchfox.org/mozilla-central/rev/97d488a17a848ce3bebbfc83dc916cf20b88451c/services/fxaccounts/FxAccounts.jsm) and [`browserid_identity.js`](https://searchfox.org/mozilla-central/rev/97d488a17a848ce3bebbfc83dc916cf20b88451c/services/sync/modules/browserid_identity.js). On Firefox for Android and Firefox for iOS, a state machine drives this process.

> There's a [diagram of the FxA state machine](/docs/assets/fxa-states.pdf).

## Sync

On Firefox for iOS, a second state machine drives the remote setup process: comparing timestamps in `info/collections`, fetching `meta/global`, and ensuring the keys in `crypto/keys` are up-to-date.

> There's a [diagram of the Sync state machine](/docs/assets/sync-states.pdf).
