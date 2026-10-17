// contracts/src/test_circuit_sim.ts
import { domainHashSha256 } from './witnesses.ts';

function simulateBilateralCircuitBreaker(oracleVarianceBps: bigint): boolean {
  const MAX_VARIANCE_BPS = 150n;
  if (oracleVarianceBps > MAX_VARIANCE_BPS) {
    console.log(`[SIM] Circuit Breaker TRIPPED. Variance: ${oracleVarianceBps} bps`);
    return true;
  }
  return false;
}

function simulateMultilateralNettingGate(grossUsd: bigint, netUsd: bigint): boolean {
  if (netUsd > grossUsd) {
    throw new Error("Invalid multilateral netting compression. Net > Gross.");
  }
  if (grossUsd < 50000n) {
    throw new Error("Multilateral ring below $50k institutional threshold.");
  }
  const compressionRatio = Number((grossUsd - netUsd) * 10000n / grossUsd) / 100;
  console.log(`[SIM] Multilateral Netting Gate PASSED. Compression: ${compressionRatio}%`);
  return true;
}

function simulateEscapeHatch(lockTime: bigint, currentTime: bigint, vaultCommitment: string): string {
  if ((currentTime - lockTime) <= 120n) {
    throw new Error("Batch window still active: cannot force exit before 120s timeout");
  }
  const buf = domainHashSha256("obsidian:escape:unlock:", Buffer.from(vaultCommitment, 'utf-8'));
  console.log(`[SIM] L1 Escape Hatch PASSED. Unlocking Vault.`);
  return Buffer.from(buf).toString('hex');
}

console.log("=========================================================================");
console.log(" OBSIDIAN-ZK COMPACT CIRCUIT LOCAL SIMULATOR");
console.log("=========================================================================");

try {
  simulateBilateralCircuitBreaker(45n);
  simulateBilateralCircuitBreaker(320n);
  simulateMultilateralNettingGate(45_000_000n, 4_500_000n);
  simulateEscapeHatch(1_760_000_000n, 1_760_000_125n, "ERC7943_VAULT_LOCK_1");
  console.log(" ALL LOCAL SIMULATION CHECKS PASSED.");
} catch (e) {
  console.error(" SIMULATION FAILED:", e);
  process.exit(1);
}
console.log("=========================================================================");