// enclave-daemon/src/main.rs
use obsidian_enclave::*;
use std::env;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;

static EPOCH_COUNTER: AtomicU64 = AtomicU64::new(42);

fn extract_json_u64(body: &str, key: &str, default: u64) -> u64 {
    let pattern = format!("\"{}\":", key);
    if let Some(pos) = body.find(&pattern) {
        let rest = &body[pos + pattern.len()..];
        let digits: String = rest
            .chars()
            .skip_while(|c| c.is_whitespace())
            .take_while(|c| c.is_ascii_digit())
            .collect();
        if let Ok(val) = digits.parse::<u64>() {
            return val;
        }
    }
    default
}

fn handle_client(mut stream: TcpStream) {
    let mut buffer = [0u8; 8192];
    let bytes_read = match stream.read(&mut buffer) {
        Ok(0) | Err(_) => return,
        Ok(n) => n,
    };
    let req = String::from_utf8_lossy(&buffer[..bytes_read]);
    let first_line = req.lines().next().unwrap_or("");

    if first_line.starts_with("OPTIONS ") {
        let resp = "HTTP/1.1 204 No Content\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\n\r\n";
        let _ = stream.write_all(resp.as_bytes());
        return;
    }

    if first_line.starts_with("GET /health") || first_line.starts_with("GET / ") {
        let pcr0 = domain_hash("obsidian:tee:pcr0:", b"OBSIDIAN_NITRO_ENCLAVE_PROD_V3.0");
        let proof_srv = env::var("MIDNIGHT_PROOF_SERVER_URL")
            .unwrap_or_else(|_| "http://localhost:6300".to_string());
        let body = format!(
            "{{\"status\":\"OK\",\"service\":\"obsidian-zk-enclave-v3.0\",\"midnight_proof_server\":\"{}\",\"nitro_pcr0\":\"0x{}\"}}",
            proof_srv,
            to_hex(&pcr0)
        );
        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = stream.write_all(resp.as_bytes());
        return;
    }

    if first_line.starts_with("POST /api/v1/batch-cross") {
        let body_start = req.find("\r\n\r\n").map(|i| i + 4).unwrap_or(0);
        let req_body = &req[body_start..];

        let size_usd = extract_json_u64(req_body, "size_usd", 10_000_000);
        let variance_bps = extract_json_u64(req_body, "variance_bps", 35);
        let capital_usd = extract_json_u64(req_body, "capital_usd", 50_000_000);
        let oracle_age_secs = extract_json_u64(req_body, "oracle_age_secs", 15);

        let raw_maker = format!(
            "8=FIX.4.4|35=D|11=APOLLO-77|49=APOLLO_CREDIT|55=CENTRIFUGE-SR-NOTE|54=2|38={}|44=9840|9001={}|9002=SECURITIZE_KYC_APOLLO|9003=884422|10=211",
            size_usd, capital_usd
        );
        let raw_taker = format!(
            "8=FIX.4.4|35=D|11=WMUTE-19|49=WINTERMUTE_INST|55=CENTRIFUGE-SR-NOTE|54=1|38={}|44=9920|9001={}|9002=SECURITIZE_KYC_WMUTE|9003=339911|10=198",
            size_usd, capital_usd
        );

        let maker = parse_fix_44_order(&raw_maker).unwrap();
        let taker = parse_fix_44_order(&raw_taker).unwrap();

        let base_time = 1_760_445_000u64;
        let oracle = NavOracleAttestation {
            symbol: "CENTRIFUGE-SR-NOTE".to_string(),
            nav_price_bps: 10000,
            credit_variance_bps: variance_bps,
            timestamp_secs: base_time,
            is_signature_valid: true,
        };

        let expected_pcr0 = domain_hash("obsidian:tee:pcr0:", b"OBSIDIAN_NITRO_ENCLAVE_PROD_V3.0");
        let approved_kyc = vec![maker.kyc_token, taker.kyc_token];
        let eth_lock = domain_hash("eth:erc7943:vault:", b"ETH_MAINNET_ERC7943_VAULT_LOCK");
        let next_epoch = EPOCH_COUNTER.fetch_add(1, Ordering::SeqCst) + 1;

        match execute_enclave_batch_cross(
            next_epoch,
            &maker,
            &taker,
            &oracle,
            base_time + oracle_age_secs,
            &expected_pcr0,
            &expected_pcr0,
            &approved_kyc,
            &eth_lock,
            5,
        ) {
            Ok(b) => {
                let rec_key = reconstruct_viewing_key_2_of_3(&[
                    b.selective_disclosure_shares[1].clone(),
                    b.selective_disclosure_shares[2].clone(),
                ])
                .unwrap_or(0);

                let json = format!(
                    "{{\"ok\":true,\"pool_state\":\"SETTLED\",\"epoch_id\":{},\"matched_size_usd\":{},\"clearing_price_bps\":{},\"pedersen_commitment\":\"{}\",\"public_batch_root\":\"0x{}\",\"erc7943_dvp_release_hash\":\"0x{}\",\"sponsored_dust_burned\":{},\"reconstructed_viewing_key\":\"{}\"}}",
                    b.epoch_id,
                    b.matched_size_usd,
                    b.clearing_price_bps,
                    b.homomorphic_size_commitment.value,
                    to_hex(&b.public_batch_root),
                    to_hex(&b.erc7943_dvp_release_hash),
                    b.sponsored_dust_burned,
                    rec_key
                );
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\n\r\n{}",
                    json.len(),
                    json
                );
                let _ = stream.write_all(resp.as_bytes());
            }
            Err(err) => {
                let reason = format!("{:?}", err);
                let json = format!(
                    "{{\"ok\":false,\"pool_state\":\"REJECTED_OR_HALTED\",\"error\":\"{}\"}}",
                    reason
                );
                let resp = format!(
                    "HTTP/1.1 422 Unprocessable Entity\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\n\r\n{}",
                    json.len(),
                    json
                );
                let _ = stream.write_all(resp.as_bytes());
            }
        }
        return;
    }

    if first_line.starts_with("POST /api/v1/multilateral-ring") {
        let body_start = req.find("\r\n\r\n").map(|i| i + 4).unwrap_or(0);
        let req_body = &req[body_start..];
        let gross_usd = extract_json_u64(req_body, "gross_usd", 45_000_000);

        let ord1 = FixOrder {
            cl_ord_id: "1".into(),
            sender_comp_id: "APOLLO".into(),
            symbol: "RWA_A".into(),
            side: Side::Buy,
            order_qty_usd: gross_usd / 3,
            limit_price_bps: 10000,
            committed_capital_usd: 50_000_000,
            kyc_token: [0; 32],
            blinding_factor: 1,
        };
        let ord2 = FixOrder {
            cl_ord_id: "2".into(),
            sender_comp_id: "WINTERMUTE".into(),
            symbol: "RWA_B".into(),
            side: Side::Buy,
            order_qty_usd: gross_usd / 3,
            limit_price_bps: 10000,
            committed_capital_usd: 50_000_000,
            kyc_token: [0; 32],
            blinding_factor: 1,
        };
        let ord3 = FixOrder {
            cl_ord_id: "3".into(),
            sender_comp_id: "GALAXY".into(),
            symbol: "RWA_C".into(),
            side: Side::Buy,
            order_qty_usd: gross_usd / 3,
            limit_price_bps: 10000,
            committed_capital_usd: 50_000_000,
            kyc_token: [0; 32],
            blinding_factor: 1,
        };

        match solve_multilateral_netting_ring(&[ord1, ord2, ord3]) {
            Ok(res) => {
                let json = format!(
                    "{{\"ok\":true,\"gross_notional_usd\":{},\"net_settlement_usd\":{},\"compression_ratio_bps\":{},\"cycle_commitment_root\":\"0x{}\"}}",
                    res.gross_notional_usd,
                    res.net_settlement_usd,
                    res.compression_ratio_bps,
                    to_hex(&res.cycle_commitment_root)
                );
                let resp = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\n\r\n{}", json.len(), json);
                let _ = stream.write_all(resp.as_bytes());
            }
            Err(e) => {
                let json = format!("{{\"ok\":false,\"error\":\"{:?}\"}}", e);
                let resp = format!("HTTP/1.1 422 Unprocessable Entity\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\n\r\n{}", json.len(), json);
                let _ = stream.write_all(resp.as_bytes());
            }
        }
        return;
    }

    if first_line.starts_with("POST /api/v1/escape-hatch") {
        let commitment = domain_hash("eth:erc7943:vault:", b"LOCK");
        let res = force_unilateral_exit_escape_hatch(1000, 1150, &commitment).unwrap();
        let json = format!(
            "{{\"ok\":true,\"escape_hatch_receipt\":\"0x{}\"}}",
            to_hex(&res)
        );
        let resp = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\n\r\n{}", json.len(), json);
        let _ = stream.write_all(resp.as_bytes());
        return;
    }

    let not_found = "{\"error\":\"Not Found\"}";
    let resp = format!(
        "HTTP/1.1 404 Not Found\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\n\r\n{}",
        not_found.len(),
        not_found
    );
    let _ = stream.write_all(resp.as_bytes());
}

fn main() {
    let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let bind_addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&bind_addr).expect("Failed to bind TCP port");
    println!("=========================================================================");
    println!(
        " OBSIDIAN-ZK v3.0: HTTP/FIX ENCLAVE DAEMON LISTENING ON {}",
        bind_addr
    );
    println!(" Healthcheck     : GET  http://{}/health", bind_addr);
    println!(
        " Batch Cross     : POST http://{}/api/v1/batch-cross",
        bind_addr
    );
    println!(
        " Multilateral    : POST http://{}/api/v1/multilateral-ring",
        bind_addr
    );
    println!(
        " Escape Hatch    : POST http://{}/api/v1/escape-hatch",
        bind_addr
    );
    println!("=========================================================================");

    for s in listener.incoming().flatten() {
        thread::spawn(move || {
            handle_client(s);
        });
    }
}
