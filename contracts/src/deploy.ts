// contracts/src/deploy.ts
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import * as Rx from 'rxjs';
import { WebSocket } from 'ws';

import { CompiledContract, type ProvableCircuitId } from '@midnight-ntwrk/compact-js';
import * as ledger from '@midnight-ntwrk/ledger-v8';
import { unshieldedToken } from '@midnight-ntwrk/ledger-v8';
import { deployContract } from '@midnight-ntwrk/midnight-js/contracts';
import { getNetworkId, setNetworkId } from '@midnight-ntwrk/midnight-js/network-id';
import {
  type MidnightProvider,
  type MidnightProviders,
  type WalletProvider,
} from '@midnight-ntwrk/midnight-js/types';
import { toHex } from '@midnight-ntwrk/midnight-js/utils';
import { httpClientProofProvider } from '@midnight-ntwrk/midnight-js-http-client-proof-provider';
import { indexerPublicDataProvider } from '@midnight-ntwrk/midnight-js-indexer-public-data-provider';
import { levelPrivateStateProvider } from '@midnight-ntwrk/midnight-js-level-private-state-provider';
import { NodeZkConfigProvider } from '@midnight-ntwrk/midnight-js-node-zk-config-provider';
import {
  MidnightBech32m,
  ShieldedAddress,
  ShieldedCoinPublicKey,
  ShieldedEncryptionPublicKey,
} from '@midnight-ntwrk/wallet-sdk-address-format';
import { DustWallet } from '@midnight-ntwrk/wallet-sdk-dust-wallet';
import { WalletFacade } from '@midnight-ntwrk/wallet-sdk-facade';
import { generateRandomSeed, HDWallet, Roles } from '@midnight-ntwrk/wallet-sdk-hd';
import { ShieldedWallet } from '@midnight-ntwrk/wallet-sdk-shielded';
import {
  createKeystore,
  InMemoryTransactionHistoryStorage,
  PublicKey,
  type UnshieldedKeystore,
  UnshieldedWallet,
} from '@midnight-ntwrk/wallet-sdk-unshielded-wallet';

import * as RwaDarkpool from './managed/rwa_darkpool/contract/index.js';
import {
  type DarkPoolPrivateState,
  emptyPrivateState,
  pad32,
  witnesses,
} from './witnesses.ts';

// Enable WebSocket in Node.js for Apollo GraphQL subscriptions (wallet sync)
(globalThis as any).WebSocket = WebSocket;

const currentDir = path.dirname(fileURLToPath(import.meta.url));
const contractsRoot = path.resolve(currentDir, '..');
const envFilePath = path.resolve(contractsRoot, '.env');
const zkConfigPath = path.resolve(currentDir, 'managed', 'rwa_darkpool');
const deploymentOutputPath = path.resolve(contractsRoot, 'deployment-preprod.json');

export type DarkPoolContract = RwaDarkpool.Contract<DarkPoolPrivateState>;
export type DarkPoolCircuits = ProvableCircuitId<DarkPoolContract>;
export const DarkPoolPrivateStateId = 'obsidianDarkPoolPrivateState';
export type DarkPoolProviders = MidnightProviders<
  DarkPoolCircuits,
  typeof DarkPoolPrivateStateId,
  DarkPoolPrivateState
>;

export interface DeployConfig {
  readonly network: 'preprod' | 'preview' | 'undeployed';
  readonly indexer: string;
  readonly indexerWS: string;
  readonly node: string;
  readonly proofServer: string;
}

function loadEnvFile(filePath: string): void {
  if (!fs.existsSync(filePath)) return;
  const content = fs.readFileSync(filePath, 'utf-8');
  for (const rawLine of content.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (!line || line.startsWith('#')) continue;
    const eqIdx = line.indexOf('=');
    if (eqIdx === -1) continue;
    const key = line.slice(0, eqIdx).trim();
    const val = line.slice(eqIdx + 1).trim().replace(/^['"]|['"]$/g, '');
    if (key && process.env[key] === undefined) {
      process.env[key] = val;
    }
  }
}

function persistWalletSeedToEnv(filePath: string, seedHex: string): void {
  if (fs.existsSync(filePath)) {
    const content = fs.readFileSync(filePath, 'utf-8');
    if (/^WALLET_SEED=.*$/m.test(content)) {
      const updated = content.replace(/^WALLET_SEED=.*$/m, `WALLET_SEED=${seedHex}`);
      fs.writeFileSync(filePath, updated, 'utf-8');
      return;
    }
    fs.appendFileSync(filePath, `\nWALLET_SEED=${seedHex}\n`, 'utf-8');
    return;
  }
  const examplePath = path.resolve(contractsRoot, '.env.example');
  if (fs.existsSync(examplePath)) {
    const template = fs.readFileSync(examplePath, 'utf-8');
    const populated = template.replace(/^WALLET_SEED=.*$/m, `WALLET_SEED=${seedHex}`);
    fs.writeFileSync(filePath, populated, 'utf-8');
  } else {
    fs.writeFileSync(filePath, `WALLET_SEED=${seedHex}\n`, 'utf-8');
  }
}

function resolveConfig(): DeployConfig {
  const networkRaw = (process.env.MIDNIGHT_NETWORK ?? 'preprod').toLowerCase();
  const network: 'preprod' | 'preview' | 'undeployed' =
    networkRaw === 'preview' || networkRaw === 'undeployed' ? networkRaw : 'preprod';

  setNetworkId(network);

  if (network === 'undeployed') {
    return {
      network,
      indexer: process.env.MIDNIGHT_INDEXER_URL ?? 'http://127.0.0.1:8088/api/v3/graphql',
      indexerWS: process.env.MIDNIGHT_INDEXER_WS_URL ?? 'ws://127.0.0.1:8088/api/v3/graphql/ws',
      node: process.env.MIDNIGHT_NODE_URL ?? 'http://127.0.0.1:9944',
      proofServer: process.env.MIDNIGHT_PROOF_SERVER_URL ?? 'http://127.0.0.1:6300',
    };
  }

  if (network === 'preview') {
    return {
      network,
      indexer: process.env.MIDNIGHT_INDEXER_URL ?? 'https://indexer.preview.midnight.network/api/v3/graphql',
      indexerWS: process.env.MIDNIGHT_INDEXER_WS_URL ?? 'wss://indexer.preview.midnight.network/api/v3/graphql/ws',
      node: process.env.MIDNIGHT_NODE_URL ?? 'https://rpc.preview.midnight.network',
      proofServer: process.env.MIDNIGHT_PROOF_SERVER_URL ?? 'http://127.0.0.1:6300',
    };
  }

  const blockfrostProjectId = process.env.BLOCKFROST_PROJECT_ID?.trim();
  const preprodDefaultIndexer = blockfrostProjectId
    ? `https://midnight-preprod.blockfrost.io/api/v0?project_id=${blockfrostProjectId}`
    : 'https://indexer.preprod.midnight.network/api/v3/graphql';
  const preprodDefaultIndexerWs = blockfrostProjectId
    ? `wss://midnight-preprod.blockfrost.io/api/v0/ws?project_id=${blockfrostProjectId}`
    : 'wss://indexer.preprod.midnight.network/api/v3/graphql/ws';
  const preprodDefaultNode = blockfrostProjectId
    ? `https://rpc.midnight-preprod.blockfrost.io?project_id=${blockfrostProjectId}`
    : 'https://rpc.preprod.midnight.network';

  return {
    network: 'preprod',
    indexer: process.env.MIDNIGHT_INDEXER_URL ?? preprodDefaultIndexer,
    indexerWS: process.env.MIDNIGHT_INDEXER_WS_URL ?? preprodDefaultIndexerWs,
    node: process.env.MIDNIGHT_NODE_URL ?? preprodDefaultNode,
    proofServer: process.env.MIDNIGHT_PROOF_SERVER_URL ?? 'http://127.0.0.1:6300',
  };
}

function parseBytes32Env(envVar: string, fallbackTag: string): Uint8Array {
  const raw = process.env[envVar]?.trim();
  if (!raw) {
    return pad32(fallbackTag);
  }
  const cleanHex = raw.startsWith('0x') ? raw.slice(2) : raw;
  if (/^[0-9a-fA-F]{64}$/.test(cleanHex)) {
    return new Uint8Array(Buffer.from(cleanHex, 'hex'));
  }
  return pad32(raw);
}

export interface ConstructorCommitments {
  enclavePcr0Preimage: Uint8Array;
  oracleSecretPreimage: Uint8Array;
  makerKycSecret: Uint8Array;
  takerKycSecret: Uint8Array;
  initKycRoot: Uint8Array;
  initOraclePkHash: Uint8Array;
  initEnclavePcr0: Uint8Array;
  initAgentViewingPk: Uint8Array;
}

export function buildConstructorCommitments(): ConstructorCommitments {
  const enclavePcr0Preimage = parseBytes32Env(
    'ENCLAVE_PCR0_PREIMAGE',
    'obsidian-aws-nitro-enclave-pcr0',
  );
  const oracleSecretPreimage = parseBytes32Env(
    'ORACLE_SECRET_PREIMAGE',
    'obsidian-nav-oracle-secret-v3',
  );
  const makerKycSecret = parseBytes32Env('MAKER_KYC_SECRET_PREIMAGE', 'obsidian-inst-maker-kyc-v3');
  const takerKycSecret = parseBytes32Env('TAKER_KYC_SECRET_PREIMAGE', 'obsidian-inst-taker-kyc-v3');
  const agentViewingPreimage = parseBytes32Env(
    'TRANSFER_AGENT_VIEWING_PREIMAGE',
    'obsidian-securitize-ta-view-v3',
  );

  // Use the compiled Compact pure circuits so on-chain persistentHash matches 100%
  const initEnclavePcr0 = RwaDarkpool.pureCircuits.compute_domain_hash(
    pad32('obsidian:tee:pcr0:'),
    enclavePcr0Preimage,
  );
  const initOraclePkHash = RwaDarkpool.pureCircuits.compute_domain_hash(
    pad32('obsidian:oracle:pk:'),
    oracleSecretPreimage,
  );
  const makerKycLeaf = RwaDarkpool.pureCircuits.compute_domain_hash(
    pad32('obsidian:kyc:v1:'),
    makerKycSecret,
  );
  const takerKycLeaf = RwaDarkpool.pureCircuits.compute_domain_hash(
    pad32('obsidian:kyc:v1:'),
    takerKycSecret,
  );
  const initKycRoot = RwaDarkpool.pureCircuits.compute_triple_hash(
    pad32('obsidian:kyc:root:'),
    makerKycLeaf,
    takerKycLeaf,
  );
  const initAgentViewingPk = RwaDarkpool.pureCircuits.compute_domain_hash(
    pad32('obsidian:agent:pk:'),
    agentViewingPreimage,
  );

  return {
    enclavePcr0Preimage,
    oracleSecretPreimage,
    makerKycSecret,
    takerKycSecret,
    initKycRoot,
    initOraclePkHash,
    initEnclavePcr0,
    initAgentViewingPk,
  };
}

const deriveKeysFromSeed = (seed: string) => {
  const hdWallet = HDWallet.fromSeed(Buffer.from(seed, 'hex'));
  if (hdWallet.type !== 'seedOk') {
    throw new Error('Failed to initialize HDWallet from seed (must be valid hex seed)');
  }

  const derivationResult = hdWallet.hdWallet
    .selectAccount(0)
    .selectRoles([Roles.Zswap, Roles.NightExternal, Roles.Dust])
    .deriveKeysAt(0);

  if (derivationResult.type !== 'keysDerived') {
    throw new Error('Failed to derive HD wallet keys');
  }

  hdWallet.hdWallet.clear();
  return derivationResult.keys;
};

export interface WalletContext {
  wallet: WalletFacade;
  shieldedSecretKeys: ledger.ZswapSecretKeys;
  dustSecretKey: ledger.DustSecretKey;
  unshieldedKeystore: UnshieldedKeystore;
}

const signTransactionIntents = (
  tx: { intents?: Map<number, any> },
  signFn: (payload: Uint8Array) => ledger.Signature,
  proofMarker: 'proof' | 'pre-proof',
): void => {
  if (!tx.intents || tx.intents.size === 0) return;

  for (const segment of tx.intents.keys()) {
    const intent = tx.intents.get(segment);
    if (!intent) continue;

    const cloned = ledger.Intent.deserialize<
      ledger.SignatureEnabled,
      ledger.Proofish,
      ledger.PreBinding
    >('signature', proofMarker, 'pre-binding', intent.serialize());

    const sigData = cloned.signatureData(segment);
    const signature = signFn(sigData);

    if (cloned.fallibleUnshieldedOffer) {
      const sigs = cloned.fallibleUnshieldedOffer.inputs.map(
        (_: ledger.UtxoSpend, i: number) =>
          cloned.fallibleUnshieldedOffer!.signatures.at(i) ?? signature,
      );
      cloned.fallibleUnshieldedOffer = cloned.fallibleUnshieldedOffer.addSignatures(sigs);
    }

    if (cloned.guaranteedUnshieldedOffer) {
      const sigs = cloned.guaranteedUnshieldedOffer.inputs.map(
        (_: ledger.UtxoSpend, i: number) =>
          cloned.guaranteedUnshieldedOffer!.signatures.at(i) ?? signature,
      );
      cloned.guaranteedUnshieldedOffer = cloned.guaranteedUnshieldedOffer.addSignatures(sigs);
    }

    tx.intents.set(segment, cloned);
  }
};

export const createWalletAndMidnightProvider = async (
  ctx: WalletContext,
): Promise<WalletProvider & MidnightProvider> => {
  const state = await Rx.firstValueFrom(ctx.wallet.state().pipe(Rx.filter((s) => s.isSynced)));
  return {
    getCoinPublicKey() {
      return state.shielded.coinPublicKey.toHexString();
    },
    getEncryptionPublicKey() {
      return state.shielded.encryptionPublicKey.toHexString();
    },
    async balanceTx(tx, ttl?) {
      const recipe = await ctx.wallet.balanceUnboundTransaction(
        tx,
        { shieldedSecretKeys: ctx.shieldedSecretKeys, dustSecretKey: ctx.dustSecretKey },
        { ttl: ttl ?? new Date(Date.now() + 30 * 60 * 1000) },
      );

      const signFn = (payload: Uint8Array) => ctx.unshieldedKeystore.signData(payload);
      signTransactionIntents(recipe.baseTransaction, signFn, 'proof');
      if (recipe.balancingTransaction) {
        signTransactionIntents(recipe.balancingTransaction, signFn, 'pre-proof');
      }

      return ctx.wallet.finalizeRecipe(recipe);
    },
    submitTx(tx) {
      return ctx.wallet.submitTransaction(tx) as any;
    },
  };
};

const buildShieldedConfig = ({ indexer, indexerWS, node, proofServer }: DeployConfig) => ({
  networkId: getNetworkId(),
  indexerClientConnection: {
    indexerHttpUrl: indexer,
    indexerWsUrl: indexerWS,
  },
  provingServerUrl: new URL(proofServer),
  relayURL: new URL(node.replace(/^http/, 'ws')),
});

const buildUnshieldedConfig = ({ indexer, indexerWS }: DeployConfig) => ({
  networkId: getNetworkId(),
  indexerClientConnection: {
    indexerHttpUrl: indexer,
    indexerWsUrl: indexerWS,
  },
  txHistoryStorage: new InMemoryTransactionHistoryStorage(),
});

const buildDustConfig = ({ indexer, indexerWS, node, proofServer }: DeployConfig) => ({
  networkId: getNetworkId(),
  costParameters: {
    additionalFeeOverhead: 300_000_000_000_000n,
    feeBlocksMargin: 5,
  },
  indexerClientConnection: {
    indexerHttpUrl: indexer,
    indexerWsUrl: indexerWS,
  },
  provingServerUrl: new URL(proofServer),
  relayURL: new URL(node.replace(/^http/, 'ws')),
});

const darkPoolCompiledContract = CompiledContract.make<DarkPoolContract>(
  'rwa_darkpool',
  RwaDarkpool.Contract,
).pipe(
  CompiledContract.withWitnesses(witnesses),
  CompiledContract.withCompiledFileAssets(zkConfigPath),
);

async function main() {
  loadEnvFile(envFilePath);
  const config = resolveConfig();
  const args = process.argv.slice(2);
  const onlyGenerateWallet = args.includes('--generate-wallet');

  let seed = process.env.WALLET_SEED?.trim() ?? '';
  if (seed.startsWith('0x')) {
    seed = seed.slice(2);
  }

  let isNewSeed = false;
  if (!seed) {
    seed = toHex(Buffer.from(generateRandomSeed()));
    persistWalletSeedToEnv(envFilePath, seed);
    isNewSeed = true;
  }

  const keys = deriveKeysFromSeed(seed);
  const shieldedSecretKeys = ledger.ZswapSecretKeys.fromSeed(keys[Roles.Zswap]);
  const dustSecretKey = ledger.DustSecretKey.fromSeed(keys[Roles.Dust]);
  const unshieldedKeystore = createKeystore(keys[Roles.NightExternal], getNetworkId());

  const coinPubKey = ShieldedCoinPublicKey.fromHexString(
    shieldedSecretKeys.coinPublicKey.toString(),
  );
  const encPubKey = ShieldedEncryptionPublicKey.fromHexString(
    shieldedSecretKeys.encryptionPublicKey.toString(),
  );
  const shieldedAddress = MidnightBech32m.encode(
    getNetworkId(),
    new ShieldedAddress(coinPubKey, encPubKey),
  ).toString();
  const unshieldedAddress = unshieldedKeystore.getBech32Address();

  const commitments = buildConstructorCommitments();

  console.log('=========================================================================');
  console.log(' OBSIDIAN-ZK v3.0 — MIDNIGHT PREPROD DEPLOYMENT HARNESS');
  console.log('=========================================================================');
  console.log(` Network                : ${config.network}`);
  console.log(` Indexer HTTP           : ${config.indexer}`);
  console.log(` Node RPC               : ${config.node}`);
  console.log(` Proof Server           : ${config.proofServer}`);
  console.log('-------------------------------------------------------------------------');
  console.log(` Wallet Seed Status     : ${isNewSeed ? 'GENERATED & SAVED TO contracts/.env' : 'LOADED FROM contracts/.env'}`);
  console.log(` Unshielded Address     : ${unshieldedAddress}`);
  console.log(` Shielded Address       : ${shieldedAddress}`);
  console.log(` Preprod Faucet URL     : https://faucet.preprod.midnight.network/`);
  console.log('-------------------------------------------------------------------------');
  console.log(' Constructor Ledger Commitments (rwa_darkpool.compact):');
  console.log(`   init_kyc_root        : 0x${toHex(commitments.initKycRoot)}`);
  console.log(`   init_oracle_pk_hash  : 0x${toHex(commitments.initOraclePkHash)}`);
  console.log(`   init_enclave_pcr0    : 0x${toHex(commitments.initEnclavePcr0)}`);
  console.log(`   init_agent_viewing_pk: 0x${toHex(commitments.initAgentViewingPk)}`);
  console.log('=========================================================================');

  if (onlyGenerateWallet) {
    console.log('\n[NEXT STEP] Fund the Unshielded Address above with tNight at:');
    console.log('  https://faucet.preprod.midnight.network/');
    console.log('Then run: npm run deploy:preprod\n');
    return;
  }

  console.log('\n[1/5] Starting WalletFacade and syncing with Midnight Preprod...');
  const walletConfig = {
    ...buildShieldedConfig(config),
    ...buildUnshieldedConfig(config),
    ...buildDustConfig(config),
  };

  const wallet = await WalletFacade.init({
    configuration: walletConfig,
    shielded: (cfg) => ShieldedWallet(cfg).startWithSecretKeys(shieldedSecretKeys),
    unshielded: (cfg) =>
      UnshieldedWallet(cfg).startWithPublicKey(PublicKey.fromKeyStore(unshieldedKeystore)),
    dust: (cfg) =>
      DustWallet(cfg).startWithSecretKey(
        dustSecretKey,
        ledger.LedgerParameters.initialParameters().dust,
      ),
  });
  await wallet.start(shieldedSecretKeys, dustSecretKey);

  try {
    const syncedState = await Rx.firstValueFrom(
      wallet.state().pipe(
        Rx.throttleTime(5_000),
        Rx.filter((s) => s.isSynced),
      ),
    );

    let balance = syncedState.unshielded.balances[unshieldedToken().raw] ?? 0n;
    console.log(`  ✓ Synced. Current Unshielded Balance: ${balance.toLocaleString()} tNight`);

    if (balance === 0n) {
      console.log(`\n[2/5] Waiting for incoming tNight at ${unshieldedAddress}...`);
      console.log('      Request funds from https://faucet.preprod.midnight.network/');
      balance = await Rx.firstValueFrom(
        wallet.state().pipe(
          Rx.throttleTime(10_000),
          Rx.filter((s) => s.isSynced),
          Rx.map((s) => s.unshielded.balances[unshieldedToken().raw] ?? 0n),
          Rx.filter((b) => b > 0n),
        ),
      );
      console.log(`  ✓ Received funds! Balance: ${balance.toLocaleString()} tNight`);
    } else {
      console.log('[2/5] Wallet already funded with tNight.');
    }

    console.log('[3/5] Checking DUST registration & fee balance...');
    const latestState = await Rx.firstValueFrom(wallet.state().pipe(Rx.filter((s) => s.isSynced)));
    if (latestState.dust.availableCoins.length === 0) {
      const unregisteredUtxos = latestState.unshielded.availableCoins.filter(
        (coin: any) => coin.meta?.registeredForDustGeneration !== true,
      );
      if (unregisteredUtxos.length > 0) {
        console.log(
          `  Registering ${unregisteredUtxos.length} NIGHT UTXO(s) for DUST generation...`,
        );
        const recipe = await wallet.registerNightUtxosForDustGeneration(
          unregisteredUtxos,
          unshieldedKeystore.getPublicKey(),
          (payload) => unshieldedKeystore.signData(payload),
        );
        const finalized = await wallet.finalizeRecipe(recipe);
        await wallet.submitTransaction(finalized);
      }
      console.log('  Waiting for DUST tokens to accrue...');
      await Rx.firstValueFrom(
        wallet.state().pipe(
          Rx.throttleTime(5_000),
          Rx.filter((s) => s.isSynced),
          Rx.filter((s) => s.dust.balance(new Date()) > 0n),
        ),
      );
    }
    const dustState = await Rx.firstValueFrom(wallet.state().pipe(Rx.filter((s) => s.isSynced)));
    console.log(`  ✓ DUST available: ${dustState.dust.balance(new Date()).toLocaleString()} DUST`);

    console.log('[4/5] Configuring Midnight ZK & Indexer providers...');
    const ctx: WalletContext = {
      wallet,
      shieldedSecretKeys,
      dustSecretKey,
      unshieldedKeystore,
    };
    const walletAndMidnightProvider = await createWalletAndMidnightProvider(ctx);
    const zkConfigProvider = new NodeZkConfigProvider<DarkPoolCircuits>(zkConfigPath);
    const accountId = walletAndMidnightProvider.getCoinPublicKey();
    const storagePassword = `${Buffer.from(accountId, 'hex').toString('base64')}!`;

    const providers: DarkPoolProviders = {
      privateStateProvider: levelPrivateStateProvider<typeof DarkPoolPrivateStateId>({
        privateStateStoreName: 'obsidian-rwa-darkpool-private-state',
        accountId,
        privateStoragePasswordProvider: () => storagePassword,
      }),
      publicDataProvider: indexerPublicDataProvider(config.indexer, config.indexerWS),
      zkConfigProvider,
      proofProvider: httpClientProofProvider(config.proofServer, zkConfigProvider),
      walletProvider: walletAndMidnightProvider,
      midnightProvider: walletAndMidnightProvider,
    };

    console.log('[5/5] Deploying rwa_darkpool.compact to Midnight...');
    const initialPrivateState: DarkPoolPrivateState = {
      ...emptyPrivateState(),
      enclavePcr0Preimage: commitments.enclavePcr0Preimage,
    };

    const deployed = await deployContract(providers, {
      compiledContract: darkPoolCompiledContract,
      privateStateId: DarkPoolPrivateStateId,
      initialPrivateState,
      args: [
        commitments.initKycRoot,
        commitments.initOraclePkHash,
        commitments.initEnclavePcr0,
        commitments.initAgentViewingPk,
      ],
    });

    const contractAddress = deployed.deployTxData.public.contractAddress;
    const txId = deployed.deployTxData.public.txId;
    const blockHeight = deployed.deployTxData.public.blockHeight;

    const receipt = {
      contract: 'rwa_darkpool.compact',
      network: config.network,
      contractAddress,
      txId,
      blockHeight: Number(blockHeight),
      deployedAt: new Date().toISOString(),
      deployerUnshieldedAddress: unshieldedAddress,
      constructorArgs: {
        init_kyc_root: `0x${toHex(commitments.initKycRoot)}`,
        init_oracle_pk_hash: `0x${toHex(commitments.initOraclePkHash)}`,
        init_enclave_pcr0: `0x${toHex(commitments.initEnclavePcr0)}`,
        init_agent_viewing_pk: `0x${toHex(commitments.initAgentViewingPk)}`,
      },
    };

    fs.writeFileSync(deploymentOutputPath, JSON.stringify(receipt, null, 2), 'utf-8');

    console.log('\n=========================================================================');
    console.log(' ✓ MIDNIGHT CONTRACT DEPLOYED SUCCESSFULLY');
    console.log('=========================================================================');
    console.log(` Contract Address : ${contractAddress}`);
    console.log(` Transaction ID   : ${txId}`);
    console.log(` Block Height     : ${blockHeight}`);
    console.log(` Receipt Saved    : ${deploymentOutputPath}`);
    console.log('=========================================================================\n');
  } finally {
    await wallet.stop();
  }
}

main().catch((err) => {
  console.error('\n[FATAL] Midnight deployment failed:', err);
  process.exit(1);
});
