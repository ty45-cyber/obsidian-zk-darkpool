// contracts/src/witnesses.ts
import { createHash } from 'node:crypto';
import { pathToFileURL } from 'node:url';

export interface OrderWitness {
  trader_sk: Uint8Array;
  asset_id: Uint8Array;
  limit_price_bps: bigint;
  size_usd: bigint;
  committed_capital_usd: bigint;
  kyc_credential_secret: Uint8Array;
  blinding_factor: Uint8Array;
}

export interface OracleWitness {
  asset_id: Uint8Array;
  nav_price_bps: bigint;
  credit_variance_bps: bigint;
  timestamp_secs: bigint;
  oracle_signature_secret: Uint8Array;
}

export interface DarkPoolPrivateState {
  makerOrder: OrderWitness;
  takerOrder: OrderWitness;
  oracleAttestation: OracleWitness;
  enclavePcr0Preimage: Uint8Array;
  erc7943VaultLockHash: Uint8Array;
}

export function pad32(tag: string): Uint8Array {
  const buf = Buffer.alloc(32, 0);
  Buffer.from(tag, 'utf-8').copy(buf);
  return new Uint8Array(buf);
}

export function domainHashSha256(domain: string, payload: Uint8Array): Uint8Array {
  const hash = createHash('sha256');
  hash.update(Buffer.from(domain, 'utf-8'));
  hash.update(payload);
  return new Uint8Array(hash.digest());
}

// Matches the compiler-generated `Witnesses<PS>` type: each witness receives a
// WitnessContext and must return a `[nextPrivateState, value]` tuple.
type WitnessCtx = { privateState: DarkPoolPrivateState };

export const witnesses = {
  get_maker_order: ({ privateState }: WitnessCtx): [DarkPoolPrivateState, OrderWitness] =>
    [privateState, privateState.makerOrder],
  get_taker_order: ({ privateState }: WitnessCtx): [DarkPoolPrivateState, OrderWitness] =>
    [privateState, privateState.takerOrder],
  get_oracle_attestation: ({ privateState }: WitnessCtx): [DarkPoolPrivateState, OracleWitness] =>
    [privateState, privateState.oracleAttestation],
  get_enclave_pcr0_preimage: ({ privateState }: WitnessCtx): [DarkPoolPrivateState, Uint8Array] =>
    [privateState, privateState.enclavePcr0Preimage],
  get_erc7943_vault_lock_hash: ({ privateState }: WitnessCtx): [DarkPoolPrivateState, Uint8Array] =>
    [privateState, privateState.erc7943VaultLockHash],
};

/** @deprecated kept for backwards compatibility; state now flows through WitnessContext. */
export const createDarkPoolWitnesses = (_state?: DarkPoolPrivateState) => witnesses;

const zero32 = () => new Uint8Array(32);
const emptyOrder = (): OrderWitness => ({
  trader_sk: zero32(),
  asset_id: zero32(),
  limit_price_bps: 0n,
  size_usd: 0n,
  committed_capital_usd: 0n,
  kyc_credential_secret: zero32(),
  blinding_factor: zero32(),
});

/** Placeholder private state used at deploy time (constructor calls no witnesses). */
export const emptyPrivateState = (): DarkPoolPrivateState => ({
  makerOrder: emptyOrder(),
  takerOrder: emptyOrder(),
  oracleAttestation: {
    asset_id: zero32(),
    nav_price_bps: 0n,
    credit_variance_bps: 0n,
    timestamp_secs: 0n,
    oracle_signature_secret: zero32(),
  },
  enclavePcr0Preimage: zero32(),
  erc7943VaultLockHash: zero32(),
});


if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  console.log("=========================================================================");
  console.log(" WITNESS: FIPS-180-4 SHA-256 PARITY VERIFICATION");
  const testDigest = domainHashSha256("obsidian:test:", Buffer.from("data"));
  const expected = "ba74dab4cbe7194f4f514414c2b984e6deb4c4281e523e05470f805f11104134";
  const hex = Buffer.from(testDigest).toString('hex');
  if (hex === expected) {
    console.log(` SUCCESS: ${hex}`);
  } else {
    console.error(` FAILURE: Expected ${expected}, got ${hex}`);
    process.exit(1);
  }
  console.log("=========================================================================");
}