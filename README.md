# ObsidianZK v3.0: Institutional RWA Dark Pool & Multilateral Clearinghouse on Midnight

**Engineered by BTech Solutions** | **Architecture:** Midnight `Compact` v0.23+ / Rust AWS Nitro TEE (FIX 4.4) / ERC-7943 Atomic DvP

ObsidianZK is a zero-knowledge institutional clearinghouse and settlement rail for tokenized Real-World Assets (Private Credit, Tokenized Treasuries, and Corporate Debt). It eliminates predatory front-running on public order books, bypasses 30-to-90-day primary issuer redemption gates, and compresses institutional balance-sheet pre-funding by up to 86% via **Zero-Knowledge Multilateral Cycle Netting**.

---

## 1. The $39.02B RWA Liquidity Bottleneck

As of October 2026, distributed on-chain RWA value stands at **$39.02 Billion** ($15.65B in Tokenized Treasuries, $7.96B in Private Credit, $4.85B in Commodities, and $2.92B in Equities), yet **56% of all on-chain RWA capital sits completely dormant**:
1. **Public Order Books Leak Alpha:** Broadcasting a $10M private credit block order on a transparent DEX telegraphs distress to HFT firms, causing severe price impact.
2. **Bilateral Pre-Funding Destroys ROE:** Standard bilateral dark pools require 100% gross cash pre-funding, creating unacceptable cash drag for institutional desks and prime brokers.
3. **Opaque Privacy Pools Violate SEC/FINRA Rules:** Regulated transfer agents (Securitize, Oasis Pro) cannot clear trades inside un-auditable mixers due to mandatory Suspicious Activity Report (SAR) auditability requirements.

---

## 2. Core v3.0 Architectural Primitives

*   **Sealed-Bid ZK-Batch Auctions & Multilateral Cycle Netting:** Replaces race-prone continuous limit order books with discrete batch auctions and N-party directed cycle compression ($G = (V, E)$), settling multi-asset RWA rotations on a **net residual basis** while charging clearing fees on gross matched volume.
*   **Anti-Probing Institutional Gates:** Enforces a **$250,000 minimum committed capital floor** and a **$50,000 minimum block size** inside the Midnight `Compact` circuit, rendering HFT order-book probing economically irrational.
*   **(2-of-3) Programmable Selective Disclosure:** Uses Shamir Secret Sharing over $\mathbb{F}_p$ ($p = 2^{64} - 59$) bound to Midnight selective disclosure commitments. Market makers see zero counterparty data, while any 2 of `{Institutional Trader, RWA Transfer Agent, Designated Regulator}` can mathematically reconstruct the audit key on demand.
*   **2.5% Yield-Bearing Slashing Margin Bond:** Eliminates "free-option" settlement defaults in multilateral rings by requiring a 2.5% maintenance margin bond held in yield-bearing Tokenized Treasuries (BUIDL/USYC), automatically slashed 80/20 to affected counterparties and the protocol reserve upon DvP failure.
*   **120-Second Trustless L1 Escape Hatch (`force_unilateral_exit`):** Guarantees institutional liveness. If the TEE enclave goes offline or censors an order for more than 120 seconds, traders can unilaterally unlock their native ERC-7943 collateral directly via the Midnight L1 contract.

---

## 3. Repository Structure

```text
obsidian-zk-darkpool/
├── README.md                                  # Executive overview, quickstart & judge verification commands
├── AKINDO_SUBMISSION.md                       # Copy-paste Akindo portal answers, $1.12B model & 3-min video script
├── ARCHITECTURE_AND_ROADMAP.md                # Formal threat model, multilateral netting math & Q1 2027 roadmap
├── docker-compose.yml                         # Orchestrates midnightnetwork/proof-server:6300 + enclave-daemon:8080
├── .gitignore                                 # Excludes build artifacts, node_modules, and local keys
├── .github/workflows/ci-verification.yml      # Automated CI pipeline for Rust enclave & TypeScript witnesses
├── contracts/                                 # Midnight Compact v0.23+ circuits & TypeScript witness harness
├── enclave-daemon/                            # Railway-ready Rust FIX 4.4, Pedersen, Shamir & Netting HTTP daemon
├── cross-chain-vaults/                        # Ethereum ERC-7943 atomic DvP escrow & margin bond slashing contracts
└── dashboard/                                 # Vercel-ready Split-Screen Institutional & Auditor Telemetry Terminal