# citrea-e2e

E2E Test framework for citrea

Citrea prover configs use `proving_mode` to select SP1 execution or proving. The
default, `execute`, runs the guest and returns an unverified development proof.
`prove` uses Succinct's prover network; `prove-with-fakes` uses network proving
when sampled and execution otherwise. Network proving requires
`NETWORK_PRIVATE_KEY` in the prover node's environment and uses the SP1 SDK's
default request policy. `[sp1_host]` only has the optional `tx_backup_dir`.

For local tests, use the default `execute` mode and `PARALLEL_PROOF_LIMIT=1`
(already set by the framework). Prover containers get 8 GiB of shared memory for
SP1 execution.
