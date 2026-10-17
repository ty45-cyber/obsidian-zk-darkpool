# ObsidianZK v3.0 — Formal Threat Model, Mathematical Specification & Venture Roadmap

## 1. System Topology & Cryptographic Data Flow

```text
+-----------------------------------------------------------------------------------+
|                   INSTITUTIONAL TRADING DESKS & PRIME BROKERS                     |
|         Apollo Credit | Wintermute RWA | Galaxy Prime | Securitize Agent          |
+-----------------------------------------+-----------------------------------------+
                                          |
       1. Standard FIX 4.4 (35=D) Wire    |  Client-Side Pedersen Blinding:
          Tags 9001 (Cap), 9002 (ZK-KYC)  |  C(v, r) = (g^v * h^r) mod p
                                          v
+-----------------------------------------------------------------------------------+
|            OBSIDIAN-ZK RUST TEE ENCLAVE DAEMON (AWS NITRO / RAILWAY)              |
|  - Hardware Attestation: Verifies PCR0 measurement against Compact ledger root    |
|  - Multilateral Cycle Solver: Compresses N-desk RWA rings into net obligations    |
|  - (2-of-3) Shamir Splitter: Shards batch viewing key over F_p (Trader/Agent/SEC) |
|  - DUST Paymaster Relay: Sponsors Midnight DUST gas against USDC fee deduction    |
+-----------------------------------------+-----------------------------------------+
                                          |
       2. Private Witness Injection       |  Zero Plaintext Price/Size on Public L1
          (witnesses.ts / Proof Server)   v
+-----------------------------------------------------------------------------------+
|              MIDNIGHT NETWORK COMPACT SMART CONTRACT (rwa_darkpool.compact)       |
|  - Enforces >= $250k Capital Floor & >= $50k Minimum Block Size                   |
|  - Verifies ZK-Oracle Heartbeat (<= 300s) & Credit Variance Breaker (<= 150 bps)  |
|  - Publishes SHA-256 Batch Root, Netting Compression Telemetry & Audit Receipt    |
|  - Provides 120s Trustless L1 Escape Hatch (force_unilateral_exit)                |
+-----------------------------------------+-----------------------------------------+
                                          |
       3. Cryptographic Settlement Root   |  Bridge-Free Lock-and-Attest Verification
                                          v
+-----------------------------------------------------------------------------------+
|          NATIVE ETHEREUM / SOLANA ERC-7943 ESCROW & MARGIN BOND VAULTS            |
|  - Executes Atomic T+0 DvP on host chain without bridging RWA principal           |
|  - Enforces 2.5% BUIDL/USYC Slashing Margin Bond (80% Make-Whole / 20% Reserve)   |
+-----------------------------------------------------------------------------------+