# Security Policy

This repository holds the Soroban contracts that sit on Konfirm's payment and settlement path. If you find a security issue, please report it privately rather than opening a public issue or pull request.

## Reporting a vulnerability

Use GitHub's private vulnerability reporting: open the **Security** tab of this repository and choose **Report a vulnerability**. Include the affected contract, the steps to reproduce, and the impact you expect. Please do not disclose the issue publicly until we have had a chance to respond.

## Scope

In scope:

- Authorization and access control in the `compliance`, `payment`, `treasury`, and `channel` contracts (admin checks, multisig signer and threshold logic, pause handling, `initialize` guards).
- Fund safety: escrow handling and refunds in `channel`, settlement execution in `treasury`, and double-spend or double-refund paths in `payment`.
- Signature verification and replay protection, including the `KONFIRM_CHAN_V1` domain separation and nonce checks in `channel`.
- Key handling for the deployer and treasury signer keys. The operational key procedures are documented in `konfirm-backend/docs/KEYS.md`.

Out of scope:

- Issues that require control of the admin, deployer, or treasury signer keys without a flaw in the contract logic.
- Findings against Stellar or Soroban itself; report those to the upstream projects.
- Deployment parameter choices, such as the challenge period value, which are documented as operational decisions rather than contract guarantees.

## Response

We aim to acknowledge reports within 5 business days. This is a small, actively developed project, so response and fix times may vary. We will keep you informed of progress and credit reporters who want to be credited.

## Branch protection on `main`

`main` is protected so that CI results gate every merge. The rule is declared in [`.github/branch-protection.json`](.github/branch-protection.json) and applied with:

```
gh api -X PUT repos/Konfirm-xyz/konfirm-contracts/branches/main/protection \
  --input .github/branch-protection.json
```

It requires the `test` and `audit` CI checks to pass, requires a pull request with at least one approving review, and blocks force-pushes and branch deletion. Verify it with `gh api repos/Konfirm-xyz/konfirm-contracts/branches/main/protection`.
