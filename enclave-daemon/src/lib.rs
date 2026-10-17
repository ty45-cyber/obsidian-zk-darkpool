// enclave-daemon/src/lib.rs
use std::collections::HashMap;

pub const FIELD_PRIME: u128 = 18_446_744_073_709_551_557;
pub const GENERATOR_G: u128 = 5;
pub const GENERATOR_H: u128 = 11;

pub const MIN_INSTITUTIONAL_CAPITAL_USD: u64 = 250_000;
pub const MIN_BLOCK_SIZE_USD: u64 = 50_000;
pub const MAX_ORACLE_STALENESS_SECS: u64 = 300;
pub const MAX_CREDIT_VARIANCE_BPS: u64 = 150;
pub const DEFAULT_MARGIN_SLASHING_RATE_BPS: u64 = 250; // 2.5% of Gross Volume

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Side {
    Buy,
    Sell,
}

#[derive(Debug, Clone)]
pub struct FixOrder {
    pub cl_ord_id: String,
    pub sender_comp_id: String,
    pub symbol: String,
    pub side: Side,
    pub order_qty_usd: u64,
    pub limit_price_bps: u64,
    pub committed_capital_usd: u64,
    pub kyc_token: [u8; 32],
    pub blinding_factor: u64,
}

#[derive(Debug, Clone)]
pub struct NavOracleAttestation {
    pub symbol: String,
    pub nav_price_bps: u64,
    pub credit_variance_bps: u64,
    pub timestamp_secs: u64,
    pub is_signature_valid: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PedersenCommitment {
    pub value: u128,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShamirShare {
    pub index: u64,
    pub value: u128,
}

#[derive(Debug, Clone)]
pub struct BatchSettlementBundle {
    pub epoch_id: u64,
    pub symbol: String,
    pub matched_size_usd: u64,
    pub clearing_price_bps: u64,
    pub homomorphic_size_commitment: PedersenCommitment,
    pub public_batch_root: [u8; 32],
    pub erc7943_dvp_release_hash: [u8; 32],
    pub sponsored_dust_burned: u64,
    pub protocol_fee_usdc_cents: u64,
    pub selective_disclosure_shares: [ShamirShare; 3],
}

#[derive(Debug, Clone)]
pub struct MultilateralNettingResult {
    pub participant_count: usize,
    pub gross_notional_usd: u64,
    pub net_settlement_usd: u64,
    pub compression_ratio_bps: u64, // e.g., 8250 = 82.50% capital saved
    pub cycle_commitment_root: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DarkPoolError {
    InvalidFixChecksum,
    MissingMandatoryFixTag(u32),
    UnattestedEnclavePcr0,
    OracleSignatureInvalid,
    OracleStale { age_secs: u64 },
    CircuitBreakerTripped { variance_bps: u64 },
    InsufficientInstitutionalCapital { provided_usd: u64 },
    OrderBelowMinimumBlockSize { matched_usd: u64 },
    KycVerificationFailed,
    AssetMismatch,
    PriceDidNotCross { ask_bps: u64, bid_bps: u64 },
    InsufficientShamirShares,
    EscapeHatchTimeoutNotReached,
}

pub fn sha256(data: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let bit_len = (data.len() as u64) * 8;
    let mut padded = data.to_vec();
    padded.push(0x80);
    while (padded.len() % 64) != 56 {
        padded.push(0x00);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in padded.chunks_exact(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) =
            (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }
    let mut out = [0u8; 32];
    for (i, word) in h.iter().enumerate() {
        out[i * 4..(i + 1) * 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

pub fn domain_hash(domain: &str, data: &[u8]) -> [u8; 32] {
    let mut buf = Vec::with_capacity(domain.len() + data.len());
    buf.extend_from_slice(domain.as_bytes());
    buf.extend_from_slice(data);
    sha256(&buf)
}

pub fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

pub fn mod_pow(mut base: u128, mut exp: u128, modulus: u128) -> u128 {
    let mut result: u128 = 1;
    base %= modulus;
    while exp > 0 {
        if exp & 1 == 1 {
            result = mul_mod(result, base, modulus);
        }
        exp >>= 1;
        base = mul_mod(base, base, modulus);
    }
    result
}

pub fn mul_mod(mut a: u128, mut b: u128, m: u128) -> u128 {
    let mut res: u128 = 0;
    a %= m;
    while b > 0 {
        if b & 1 == 1 {
            res = (res + a) % m;
        }
        a = (a * 2) % m;
        b >>= 1;
    }
    res
}

pub fn mod_inv(a: u128, p: u128) -> u128 {
    mod_pow(a, p - 2, p)
}

pub fn pedersen_commit(value: u64, blinding: u64) -> PedersenCommitment {
    let gv = mod_pow(GENERATOR_G, value as u128, FIELD_PRIME);
    let hr = mod_pow(GENERATOR_H, blinding as u128, FIELD_PRIME);
    PedersenCommitment {
        value: mul_mod(gv, hr, FIELD_PRIME),
    }
}

pub fn pedersen_add(c1: &PedersenCommitment, c2: &PedersenCommitment) -> PedersenCommitment {
    PedersenCommitment {
        value: mul_mod(c1.value, c2.value, FIELD_PRIME),
    }
}

pub fn split_viewing_key_2_of_3(secret_key: u128, coeff_a1: u128) -> [ShamirShare; 3] {
    let s = secret_key % FIELD_PRIME;
    let a1 = coeff_a1 % FIELD_PRIME;
    [
        ShamirShare {
            index: 1,
            value: (s + mul_mod(a1, 1, FIELD_PRIME)) % FIELD_PRIME,
        },
        ShamirShare {
            index: 2,
            value: (s + mul_mod(a1, 2, FIELD_PRIME)) % FIELD_PRIME,
        },
        ShamirShare {
            index: 3,
            value: (s + mul_mod(a1, 3, FIELD_PRIME)) % FIELD_PRIME,
        },
    ]
}

pub fn reconstruct_viewing_key_2_of_3(shares: &[ShamirShare]) -> Result<u128, DarkPoolError> {
    if shares.len() < 2 {
        return Err(DarkPoolError::InsufficientShamirShares);
    }
    let (x0, y0) = (shares[0].index as u128, shares[0].value);
    let (x1, y1) = (shares[1].index as u128, shares[1].value);

    let num0 = (FIELD_PRIME - x1) % FIELD_PRIME;
    let den0 = (FIELD_PRIME + x0 - x1) % FIELD_PRIME;
    let l0 = mul_mod(num0, mod_inv(den0, FIELD_PRIME), FIELD_PRIME);

    let num1 = (FIELD_PRIME - x0) % FIELD_PRIME;
    let den1 = (FIELD_PRIME + x1 - x0) % FIELD_PRIME;
    let l1 = mul_mod(num1, mod_inv(den1, FIELD_PRIME), FIELD_PRIME);

    let term0 = mul_mod(y0, l0, FIELD_PRIME);
    let term1 = mul_mod(y1, l1, FIELD_PRIME);
    Ok((term0 + term1) % FIELD_PRIME)
}

pub fn parse_fix_44_order(raw_fix: &str) -> Result<FixOrder, DarkPoolError> {
    let delimiter = if raw_fix.contains('\x01') {
        '\x01'
    } else {
        '|'
    };
    let mut tags: HashMap<u32, &str> = HashMap::new();

    for field in raw_fix.split(delimiter) {
        if field.is_empty() {
            continue;
        }
        if let Some((k, v)) = field.split_once('=') {
            if let Ok(tag_num) = k.trim().parse::<u32>() {
                tags.insert(tag_num, v.trim());
            }
        }
    }

    let msg_type = tags
        .get(&35)
        .ok_or(DarkPoolError::MissingMandatoryFixTag(35))?;
    if *msg_type != "D" {
        return Err(DarkPoolError::MissingMandatoryFixTag(35));
    }

    let cl_ord_id = (*tags
        .get(&11)
        .ok_or(DarkPoolError::MissingMandatoryFixTag(11))?)
    .to_string();
    let sender_comp_id = (*tags
        .get(&49)
        .ok_or(DarkPoolError::MissingMandatoryFixTag(49))?)
    .to_string();
    let symbol = (*tags
        .get(&55)
        .ok_or(DarkPoolError::MissingMandatoryFixTag(55))?)
    .to_string();

    let side_raw = *tags
        .get(&54)
        .ok_or(DarkPoolError::MissingMandatoryFixTag(54))?;
    let side = match side_raw {
        "1" => Side::Buy,
        "2" => Side::Sell,
        _ => return Err(DarkPoolError::MissingMandatoryFixTag(54)),
    };

    let order_qty_usd = tags
        .get(&38)
        .ok_or(DarkPoolError::MissingMandatoryFixTag(38))?
        .parse::<u64>()
        .map_err(|_| DarkPoolError::MissingMandatoryFixTag(38))?;

    let limit_price_bps = tags
        .get(&44)
        .ok_or(DarkPoolError::MissingMandatoryFixTag(44))?
        .parse::<u64>()
        .map_err(|_| DarkPoolError::MissingMandatoryFixTag(44))?;

    let committed_capital_usd = tags
        .get(&9001)
        .ok_or(DarkPoolError::MissingMandatoryFixTag(9001))?
        .parse::<u64>()
        .map_err(|_| DarkPoolError::MissingMandatoryFixTag(9001))?;

    let kyc_str = *tags
        .get(&9002)
        .ok_or(DarkPoolError::MissingMandatoryFixTag(9002))?;
    let kyc_token = domain_hash("obsidian:kyc:leaf:", kyc_str.as_bytes());

    let blinding_factor = tags
        .get(&9003)
        .unwrap_or(&"424242")
        .parse::<u64>()
        .unwrap_or(424242);

    Ok(FixOrder {
        cl_ord_id,
        sender_comp_id,
        symbol,
        side,
        order_qty_usd,
        limit_price_bps,
        committed_capital_usd,
        kyc_token,
        blinding_factor,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn execute_enclave_batch_cross(
    epoch_id: u64,
    maker_sell: &FixOrder,
    taker_buy: &FixOrder,
    oracle: &NavOracleAttestation,
    current_time_secs: u64,
    enclave_pcr0: &[u8; 32],
    expected_enclave_pcr0: &[u8; 32],
    approved_kyc_tokens: &[[u8; 32]],
    erc7943_ethereum_lock_tx: &[u8; 32],
    clearing_fee_bps: u64,
) -> Result<BatchSettlementBundle, DarkPoolError> {
    if enclave_pcr0 != expected_enclave_pcr0 {
        return Err(DarkPoolError::UnattestedEnclavePcr0);
    }
    if !oracle.is_signature_valid {
        return Err(DarkPoolError::OracleSignatureInvalid);
    }
    let age_secs = current_time_secs.saturating_sub(oracle.timestamp_secs);
    if age_secs > MAX_ORACLE_STALENESS_SECS {
        return Err(DarkPoolError::OracleStale { age_secs });
    }
    if oracle.credit_variance_bps > MAX_CREDIT_VARIANCE_BPS {
        return Err(DarkPoolError::CircuitBreakerTripped {
            variance_bps: oracle.credit_variance_bps,
        });
    }
    if maker_sell.symbol != taker_buy.symbol || maker_sell.symbol != oracle.symbol {
        return Err(DarkPoolError::AssetMismatch);
    }
    if maker_sell.committed_capital_usd < MIN_INSTITUTIONAL_CAPITAL_USD {
        return Err(DarkPoolError::InsufficientInstitutionalCapital {
            provided_usd: maker_sell.committed_capital_usd,
        });
    }
    if taker_buy.committed_capital_usd < MIN_INSTITUTIONAL_CAPITAL_USD {
        return Err(DarkPoolError::InsufficientInstitutionalCapital {
            provided_usd: taker_buy.committed_capital_usd,
        });
    }

    let matched_size_usd = maker_sell.order_qty_usd.min(taker_buy.order_qty_usd);
    if matched_size_usd < MIN_BLOCK_SIZE_USD {
        return Err(DarkPoolError::OrderBelowMinimumBlockSize {
            matched_usd: matched_size_usd,
        });
    }

    if !approved_kyc_tokens.contains(&maker_sell.kyc_token)
        || !approved_kyc_tokens.contains(&taker_buy.kyc_token)
    {
        return Err(DarkPoolError::KycVerificationFailed);
    }

    if maker_sell.limit_price_bps > taker_buy.limit_price_bps {
        return Err(DarkPoolError::PriceDidNotCross {
            ask_bps: maker_sell.limit_price_bps,
            bid_bps: taker_buy.limit_price_bps,
        });
    }

    let clearing_price_bps = (maker_sell.limit_price_bps + taker_buy.limit_price_bps) / 2;

    let c_maker = pedersen_commit(matched_size_usd, maker_sell.blinding_factor);
    let c_taker = pedersen_commit(matched_size_usd, taker_buy.blinding_factor);
    let combined_commitment = pedersen_add(&c_maker, &c_taker);

    let mut batch_payload = Vec::with_capacity(96);
    batch_payload.extend_from_slice(&epoch_id.to_be_bytes());
    batch_payload.extend_from_slice(&combined_commitment.value.to_be_bytes());
    batch_payload.extend_from_slice(&maker_sell.kyc_token);
    let public_batch_root = domain_hash("obsidian:batch:root:", &batch_payload);

    let mut dvp_payload = Vec::with_capacity(64);
    dvp_payload.extend_from_slice(&public_batch_root);
    dvp_payload.extend_from_slice(erc7943_ethereum_lock_tx);
    let erc7943_dvp_release_hash = domain_hash("obsidian:erc7943:dvp:", &dvp_payload);

    let sponsored_dust_burned = 1_250_000;
    let protocol_fee_usdc_cents =
        (matched_size_usd as u128 * clearing_fee_bps as u128 / 100) as u64;

    let audit_secret =
        u128::from_be_bytes(public_batch_root[0..16].try_into().unwrap()) % FIELD_PRIME;
    let shares = split_viewing_key_2_of_3(audit_secret, 987_654_321_123_456);

    Ok(BatchSettlementBundle {
        epoch_id,
        symbol: maker_sell.symbol.clone(),
        matched_size_usd,
        clearing_price_bps,
        homomorphic_size_commitment: combined_commitment,
        public_batch_root,
        erc7943_dvp_release_hash,
        sponsored_dust_burned,
        protocol_fee_usdc_cents,
        selective_disclosure_shares: shares,
    })
}

pub fn solve_multilateral_netting_ring(
    orders: &[FixOrder],
) -> Result<MultilateralNettingResult, DarkPoolError> {
    let mut gross_notional_usd: u64 = 0;
    let mut net_by_desk: HashMap<&str, i128> = HashMap::new();

    for ord in orders {
        if ord.committed_capital_usd < MIN_INSTITUTIONAL_CAPITAL_USD {
            return Err(DarkPoolError::InsufficientInstitutionalCapital {
                provided_usd: ord.committed_capital_usd,
            });
        }
        gross_notional_usd = gross_notional_usd.saturating_add(ord.order_qty_usd);
        let signed_flow = match ord.side {
            Side::Buy => -(ord.order_qty_usd as i128),
            Side::Sell => ord.order_qty_usd as i128,
        };
        *net_by_desk.entry(ord.sender_comp_id.as_str()).or_insert(0) += signed_flow;
    }

    if gross_notional_usd < MIN_BLOCK_SIZE_USD {
        return Err(DarkPoolError::OrderBelowMinimumBlockSize {
            matched_usd: gross_notional_usd,
        });
    }

    let sum_abs_imbalance: u128 = net_by_desk.values().map(|v| v.unsigned_abs()).sum();
    let net_settlement_usd = (sum_abs_imbalance / 2) as u64;

    let saved_usd = gross_notional_usd.saturating_sub(net_settlement_usd);
    let compression_ratio_bps =
        ((saved_usd as u128 * 10_000) / (gross_notional_usd as u128)) as u64;

    let cycle_commitment_root = domain_hash(
        "obsidian:multilateral:ring:",
        &gross_notional_usd.to_be_bytes(),
    );

    Ok(MultilateralNettingResult {
        participant_count: net_by_desk.len(),
        gross_notional_usd,
        net_settlement_usd,
        compression_ratio_bps,
        cycle_commitment_root,
    })
}

pub fn force_unilateral_exit_escape_hatch(
    order_lock_timestamp_secs: u64,
    current_block_time_secs: u64,
    trader_vault_commitment: &[u8; 32],
) -> Result<[u8; 32], DarkPoolError> {
    if current_block_time_secs.saturating_sub(order_lock_timestamp_secs) <= 120 {
        return Err(DarkPoolError::EscapeHatchTimeoutNotReached);
    }
    Ok(domain_hash(
        "obsidian:escape:unlock:",
        trader_vault_commitment,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fips_sha256_standard_vector() {
        let digest = sha256(b"abc");
        assert_eq!(
            to_hex(&digest),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    fn sample_environment() -> (
        FixOrder,
        FixOrder,
        NavOracleAttestation,
        [u8; 32],
        Vec<[u8; 32]>,
    ) {
        let raw_maker = "8=FIX.4.4|35=D|11=ORD-MAKER-01|49=APOLLO_CREDIT|55=CENTRIFUGE-TRANCHE-A|54=2|38=5000000|44=9850|9001=15000000|9002=KYC_SECRET_APOLLO|9003=111111|10=128";
        let raw_taker = "8=FIX.4.4|35=D|11=ORD-TAKER-01|49=WINTERMUTE_RWA|55=CENTRIFUGE-TRANCHE-A|54=1|38=5000000|44=9910|9001=25000000|9002=KYC_SECRET_WINTERMUTE|9003=222222|10=132";

        let maker = parse_fix_44_order(raw_maker).unwrap();
        let taker = parse_fix_44_order(raw_taker).unwrap();

        let oracle = NavOracleAttestation {
            symbol: "CENTRIFUGE-TRANCHE-A".to_string(),
            nav_price_bps: 10000,
            credit_variance_bps: 45,
            timestamp_secs: 1_760_440_000,
            is_signature_valid: true,
        };

        let pcr0 = domain_hash("obsidian:tee:pcr0:", b"AWS_NITRO_ENCLAVE_IMAGE_V3.0.0");
        let approved_kyc = vec![maker.kyc_token, taker.kyc_token];

        (maker, taker, oracle, pcr0, approved_kyc)
    }

    #[test]
    fn test_1_valid_institutional_5m_block_cross() {
        let (maker, taker, oracle, pcr0, kyc) = sample_environment();
        let lock_tx = domain_hash("eth:erc7943:lock:", b"0xabc123");
        let bundle = execute_enclave_batch_cross(
            1,
            &maker,
            &taker,
            &oracle,
            1_760_440_120,
            &pcr0,
            &pcr0,
            &kyc,
            &lock_tx,
            5,
        )
        .unwrap();
        assert_eq!(bundle.matched_size_usd, 5_000_000);
        assert_eq!(bundle.clearing_price_bps, 9880);
    }

    #[test]
    fn test_2_homomorphic_pedersen_commitment_integrity() {
        let size = 5_000_000u64;
        let c1 = pedersen_commit(size, 111_111);
        let c2 = pedersen_commit(size, 222_222);
        assert_eq!(pedersen_add(&c1, &c2), pedersen_commit(size * 2, 333_333));
    }

    #[test]
    fn test_3_shamir_2_of_3_selective_disclosure_reconstruction() {
        let secret: u128 = 9_182_736_455_647_382_910;
        let shares = split_viewing_key_2_of_3(secret, 123_456_789);
        assert_eq!(
            reconstruct_viewing_key_2_of_3(&[shares[0].clone(), shares[1].clone()]).unwrap(),
            secret
        );
        assert_eq!(
            reconstruct_viewing_key_2_of_3(&[shares[1].clone(), shares[2].clone()]).unwrap(),
            secret
        );
        assert_eq!(
            reconstruct_viewing_key_2_of_3(&[shares[0].clone(), shares[2].clone()]).unwrap(),
            secret
        );
    }

    #[test]
    fn test_4_rejects_hft_order_book_probing_micro_orders() {
        let (mut maker, mut taker, oracle, pcr0, kyc) = sample_environment();
        maker.order_qty_usd = 100;
        taker.order_qty_usd = 100;
        let res = execute_enclave_batch_cross(
            1,
            &maker,
            &taker,
            &oracle,
            1_760_440_100,
            &pcr0,
            &pcr0,
            &kyc,
            &[0u8; 32],
            5,
        );
        assert_eq!(
            res.unwrap_err(),
            DarkPoolError::OrderBelowMinimumBlockSize { matched_usd: 100 }
        );
    }

    #[test]
    fn test_5_rejects_under_capitalized_participant_below_250k() {
        let (maker, mut taker, oracle, pcr0, kyc) = sample_environment();
        taker.committed_capital_usd = 100_000;
        let res = execute_enclave_batch_cross(
            1,
            &maker,
            &taker,
            &oracle,
            1_760_440_100,
            &pcr0,
            &pcr0,
            &kyc,
            &[0u8; 32],
            5,
        );
        assert_eq!(
            res.unwrap_err(),
            DarkPoolError::InsufficientInstitutionalCapital {
                provided_usd: 100_000
            }
        );
    }

    #[test]
    fn test_6_trips_circuit_breaker_on_toxic_flow_credit_variance() {
        let (maker, taker, mut oracle, pcr0, kyc) = sample_environment();
        oracle.credit_variance_bps = 320;
        let res = execute_enclave_batch_cross(
            1,
            &maker,
            &taker,
            &oracle,
            1_760_440_100,
            &pcr0,
            &pcr0,
            &kyc,
            &[0u8; 32],
            5,
        );
        assert_eq!(
            res.unwrap_err(),
            DarkPoolError::CircuitBreakerTripped { variance_bps: 320 }
        );
    }

    #[test]
    fn test_7_rejects_stale_nav_oracle_attestation() {
        let (maker, taker, oracle, pcr0, kyc) = sample_environment();
        let res = execute_enclave_batch_cross(
            1,
            &maker,
            &taker,
            &oracle,
            oracle.timestamp_secs + 600,
            &pcr0,
            &pcr0,
            &kyc,
            &[0u8; 32],
            5,
        );
        assert_eq!(
            res.unwrap_err(),
            DarkPoolError::OracleStale { age_secs: 600 }
        );
    }

    #[test]
    fn test_8_rejects_unattested_enclave_pcr0_host() {
        let (maker, taker, oracle, pcr0, kyc) = sample_environment();
        let rogue_pcr0 = domain_hash("obsidian:tee:pcr0:", b"MODIFIED_MALICIOUS_BINARY");
        let res = execute_enclave_batch_cross(
            1,
            &maker,
            &taker,
            &oracle,
            1_760_440_100,
            &rogue_pcr0,
            &pcr0,
            &kyc,
            &[0u8; 32],
            5,
        );
        assert_eq!(res.unwrap_err(), DarkPoolError::UnattestedEnclavePcr0);
    }

    #[test]
    fn test_9_multilateral_netting_and_margin_slash() {
        let mut ord1 = sample_environment().0;
        let mut ord2 = sample_environment().1;
        let mut ord3 = ord1.clone();
        ord1.sender_comp_id = "DESK_A".to_string();
        ord1.side = Side::Buy;
        ord1.order_qty_usd = 15_000_000;
        ord2.sender_comp_id = "DESK_B".to_string();
        ord2.side = Side::Sell;
        ord2.order_qty_usd = 15_000_000;
        ord3.sender_comp_id = "DESK_A".to_string();
        ord3.side = Side::Sell;
        ord3.order_qty_usd = 10_000_000; // Desk A rotates

        let res = solve_multilateral_netting_ring(&[ord1, ord2, ord3]).unwrap();
        assert_eq!(res.gross_notional_usd, 40_000_000);
        // Desk A net: Buy 15M, Sell 10M -> Net +5M (Buy)
        // Desk B net: Sell 15M -> Net -15M (Sell)
        // Total abs sum = 5M + 15M = 20M. Net settlement = 10M
        assert_eq!(res.net_settlement_usd, 10_000_000);
        assert_eq!(res.compression_ratio_bps, 7500); // 75.00% capital saved

        let slash_amount = (res.gross_notional_usd * DEFAULT_MARGIN_SLASHING_RATE_BPS) / 10000;
        assert_eq!(slash_amount, 1_000_000); // 2.5% of 40M
    }

    #[test]
    fn test_10_l1_escape_hatch_timeout_verification() {
        let commitment = domain_hash("eth:erc7943:vault:", b"LOCK");
        let res_early = force_unilateral_exit_escape_hatch(1000, 1100, &commitment);
        assert_eq!(
            res_early.unwrap_err(),
            DarkPoolError::EscapeHatchTimeoutNotReached
        );

        let res_valid = force_unilateral_exit_escape_hatch(1000, 1150, &commitment);
        assert!(res_valid.is_ok());
    }
}
