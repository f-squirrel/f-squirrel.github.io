# Don't hold the key: architecture for secrets you can't afford to lose

*This is the last of three. In [part 1](./part-1-zeroize.md) we wiped secrets in memory with `zeroize` and found the copies it can't reach; in [part 2](./part-2-os-hardening.md) we stopped the OS from paging, dumping, or letting another process read them. Every trick so far shared one quiet assumption: that the plaintext key sits in your process's RAM at all. This post is about making that assumption false — which is the strongest move of the three, and the one worth reaching for when losing a key is catastrophic.*

The idea is simple to state and harder to build: arrange things so the full private key never exists in your address space. If it isn't there, there's nothing for swap, dumps, ptrace, or a missed `Drop` to leak. There are three main ways to get there, in rough order of how much they change your system.

## Don't hold the key at all: HSMs and KMS

A Hardware Security Module — or a managed KMS like AWS KMS, GCP Cloud KMS, or HashiCorp Vault's Transit engine — holds the key inside a hardened boundary and only hands you *operations*. You say "sign this digest" or "unwrap this data key," it does the work internally, and the private key never crosses into your memory. No bytes in your RAM means nothing for the previous two posts to protect. Very freeing.

The model is worth internalizing because it shapes your whole design:

- **Operations cross the boundary, keys never do.** You hold a *handle* to the key, not the key. For data encryption you typically use *envelope encryption*: the HSM/KMS holds a master key that only ever wraps and unwraps short-lived data keys, so even the working key is encrypted at rest.
- **The boundary has its own access control and audit.** Policies, quotas, and logs live with the key, not in your app — which is also why this plays nicely with the compliance paperwork.
- **It moves the threat, it doesn't delete it.** An attacker who compromises your process can't *steal* the key, but they can still ask the HSM to *use* it while they have access. So you still care about authn/authz on every signing request, rate limits, and policy. "The key can't leave" is not "the key can't be misused."

From Rust you talk to a PKCS#11 HSM with the [`cryptoki`](https://docs.rs/cryptoki) crate:

```rust
// sign without ever seeing the private key
let signature = session.sign(&Mechanism::Ecdsa, key_handle, &digest)?;
```

## Split the key: MPC and threshold signatures

With threshold signing, the key is *shared* across parties and never reassembled anywhere. Each party holds only a share; a threshold of them jointly produce a signature; the full private key is never materialized in one place. Compromise one node's memory and an attacker gets a share, not the key. It's the strongest version of "the plaintext key is never in RAM," and unlike the others it's structural rather than a single `cargo add`.

If you want to build on this, the maintained Rust building blocks are worth knowing. For threshold ECDSA (what Bitcoin and Ethereum's secp256k1 need), [`cggmp21`/`cggmp24`](https://docs.rs/cggmp21) implements the state-of-the-art CGGMP protocol and is the only audited Rust implementation under a permissive licence. For threshold Schnorr/EdDSA there's [FROST](https://www.rfc-editor.org/rfc/rfc9591.html) — the [Zcash Foundation's `frost-*` crates](https://github.com/ZcashFoundation/frost) are the reference-style choice, and `givre` is another. (Heads up: the once-popular ZenGo `multi-party-ecdsa` crate is no longer maintained and gets no security fixes, so it's not a good base for new work.)

But here's the honest part, and the reason this lives under "architecture" and not "just import a crate": **having the cryptographic primitives is not the same as having a solution.** A production threshold-signing system is a serious undertaking — you still have to design the network and message transport, authenticate every party, persist and rotate key shares, handle aborts and malicious participants, run distributed key generation, and manage presignatures safely (the `cggmp21` docs will warn you that *reusing a single presignature can leak the entire private key*). These libraries are audited and excellent, and they still leave most of the engineering — and most of the ways to shoot yourself in the foot — to you. The lesson is the same one as part 1, scaled up: lean on audited primitives, don't hand-roll the crypto, and respect that wiring them into something safe is its own hard project.

## Run it somewhere the OS can't look: TEEs

A Trusted Execution Environment — Intel SGX, AMD SEV-SNP, ARM TrustZone, AWS Nitro Enclaves — runs your signing logic in memory that's encrypted and isolated from the host OS, even from a root user or a malicious hypervisor. The key can live in cleartext *inside* the enclave while being opaque outside it.

Two things make or break a TEE deployment, and it's worth being clear-eyed about both:

- **Attestation is the actual product.** The isolation is only useful if you can *prove* to a remote party that the code running in the enclave is the code you expect, on genuine hardware. That remote attestation step is the part you must get right; without it you're trusting an unverified black box.
- **Side channels are a real, recurring track record.** TEEs — SGX especially — have a long history of microarchitectural side-channel and transient-execution attacks (the Foreshadow/ÆPIC/SGAxe lineage). They're a strong layer, not a magic box. Treat a TEE as one defense among several, keep the in-enclave code minimal, and don't let "it's in the enclave" end the threat conversation.

## A few more levers

- **Secure elements** — phone-grade Secure Enclave / StrongBox for mobile signers, where the key is generated in and never leaves dedicated hardware.
- **Ephemeral keys** — derive, use, and drop short-lived keys so even a leaked copy has a tiny window of usefulness.
- **Shamir secret sharing at rest** — split a *backup* so no single shard is sensitive on its own. (Worth a clear distinction: plain Shamir is for storage — you reconstruct the key to use it. Threshold signing above is what lets you *use* a split key without ever reassembling it.)

## Buy instead of build: commercial solutions

At institutional scale, many teams reach for a vendor instead — as much for the insurance and compliance paperwork as for the engineering. [GK8](https://www.gk8.io/products/) (now part of Galaxy) is a tidy example because its two products map straight onto this layer. [Impenetrable Vault](https://www.gk8.io/products/impenetrable-vault/) is the air-gap idea at its extreme: one-way communication with zero digital input, so the cold side signs entirely offline and stays unreachable from the internet. [uMPC](https://www.gk8.io/products/umpc/) is threshold signing productized: an unlimited number of key shards, deployable across cloud, on-premise, or HSM. Same architecture this series has been building toward — just bought rather than built.

## The whole picture: which layer stops what

The point of all three posts is that no single control is sufficient, and each one has a clear edge. Here's the map:

| Control | Stops | Doesn't help with |
|---|---|---|
| `zeroize` / `secrecy` | secret lingering in freed/dropped memory; accidental logging or serialization | live-memory reads; copies from spills, moves, realloc; swap; dumps |
| `mlock` | secret paged out to swap | hibernation image; live reads; core dumps |
| `MADV_DONTDUMP` + dump-disable | secret captured in a core dump / crash reporter | swap; live reads |
| `PR_SET_DUMPABLE` + Yama `ptrace_scope` | same-user process attaching to read live memory | root / `CAP_SYS_PTRACE` attacker |
| HSM / KMS | the key ever being in your process RAM | a compromised process still *asking* the HSM to sign |
| MPC / threshold signing | single-node compromise yielding the key | threshold-many nodes colluding or compromised; protocol misuse |
| TEE / enclave | the host OS (even root) reading enclave memory | enclave side channels; weak or missing attestation |

Read it top to bottom and you can see the shape of the whole thing: the cheap in-language controls protect against accidents and leftover copies, the OS knobs protect against the kernel moving your bytes around, and the architectural moves protect against your own process being the weak point. Each layer hands its blind spot up to the next.

## Putting it all together

Zeroization is *necessary but not sufficient* — and so is everything else here. The useful skill isn't memorizing the knobs; it's being able to look at a given leak and know which row of that table it belongs to, and which control actually addresses it.

So: start with `zeroize` on every secret, because it's free and correct. Add the OS knobs where the stakes justify the effort. And for the keys you truly can't afford to lose, change the architecture so the plaintext key is never in your process at all — and lean on audited tools to do it, because the gap between a cryptographic primitive and a safe system is exactly where this whole series lives.

← Previous: [Where `zeroize` stops: hardening keys at the OS level](./part-2-os-hardening.md)
← Start: [I Zeroized My Secret. Or Did I?](./part-1-zeroize.md)

---

*Further reading:*

- `cryptoki` (PKCS#11) — https://docs.rs/cryptoki
- `cggmp21` (threshold ECDSA) — https://docs.rs/cggmp21
- FROST — RFC 9591: https://www.rfc-editor.org/rfc/rfc9591.html · Zcash Foundation `frost` — https://github.com/ZcashFoundation/frost
- AWS Nitro Enclaves — https://docs.aws.amazon.com/enclaves/ · Intel SGX overview — https://www.intel.com/content/www/us/en/developer/tools/software-guard-extensions/overview.html
