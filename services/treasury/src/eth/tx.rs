//! EIP-1559 transaction building, signing, and key loading.

use std::sync::Arc;

use alloy_primitives::{keccak256, Address};
use k256::ecdsa::signature::hazmat::PrehashSigner;
use k256::ecdsa::{RecoveryId, Signature, SigningKey};

use super::encoding::{
    bytes_to_hex, decode_hex, rlp_encode_bytes, rlp_encode_list, rlp_encode_uint,
};
use crate::error::TxError;

// ── Signing key ──────────────────────────────────────────────────────────────

/// Load a signing key from a hex-encoded file and derive its Ethereum address.
pub fn load_signing_key(path: &str) -> (Option<Arc<SigningKey>>, Option<Address>) {
    let Ok(contents) = std::fs::read_to_string(path) else {
        return (None, None);
    };
    let hex = contents.trim().trim_start_matches("0x");
    let bytes = match decode_hex(hex) {
        Some(b) if b.len() == 32 => b,
        _ => return (None, None),
    };
    let Ok(key) = SigningKey::from_bytes(bytes.as_slice().into()) else {
        return (None, None);
    };
    let uncompressed = key.verifying_key().to_encoded_point(false);
    let hash = keccak256(&uncompressed.as_bytes()[1..]); // skip 0x04 prefix
    let addr = Address::from_slice(&hash[12..]);
    (Some(Arc::new(key)), Some(addr))
}

// ── EIP-1559 transaction building & signing ──────────────────────────────────

/// Build the RLP-encoded body shared between the signing payload and the
/// signed transaction (everything except v, r, s).
#[allow(clippy::too_many_arguments)]
fn build_tx_rlp(
    chain_id: u64,
    nonce: u64,
    max_priority_fee: u64,
    max_fee: u64,
    gas_limit: u64,
    to: &[u8],
    value: &[u8],
    data: &[u8],
) -> Vec<u8> {
    rlp_encode_list(&[
        rlp_encode_uint(chain_id),
        rlp_encode_uint(nonce),
        rlp_encode_uint(max_priority_fee),
        rlp_encode_uint(max_fee),
        rlp_encode_uint(gas_limit),
        rlp_encode_bytes(to),
        rlp_encode_bytes(value),
        rlp_encode_bytes(data),
        rlp_encode_list(&[]), // empty access list
    ])
}

/// Sign an EIP-1559 transaction and return the raw hex-encoded bytes (with `0x` prefix).
///
/// Pass `&[]` for `value` when sending 0 ETH.
#[allow(clippy::too_many_arguments)]
pub fn sign_eip1559_tx(
    chain_id: u64,
    nonce: u64,
    max_priority_fee: u64,
    max_fee: u64,
    gas_limit: u64,
    to: &[u8],
    value: &[u8],
    call_data: &[u8],
    key: &SigningKey,
) -> Result<String, TxError> {
    // 1. Build signing payload: 0x02 || rlp([chain_id, nonce, ...])
    let signing_rlp = build_tx_rlp(
        chain_id,
        nonce,
        max_priority_fee,
        max_fee,
        gas_limit,
        to,
        value,
        call_data,
    );
    let mut to_sign = vec![0x02u8];
    to_sign.extend_from_slice(&signing_rlp);
    let hash = keccak256(&to_sign);

    // 2. ECDSA sign
    let (sig, recid): (Signature, RecoveryId) = key
        .sign_prehash(hash.as_slice())
        .map_err(|e| TxError::Signing(e.to_string()))?;

    let r_bytes = sig.r().to_bytes();
    let s_bytes = sig.s().to_bytes();
    let v = recid.to_byte() as u64;

    // 3. Build final signed transaction: 0x02 || rlp([..., v, r, s])
    let signed_rlp = rlp_encode_list(&[
        rlp_encode_uint(chain_id),
        rlp_encode_uint(nonce),
        rlp_encode_uint(max_priority_fee),
        rlp_encode_uint(max_fee),
        rlp_encode_uint(gas_limit),
        rlp_encode_bytes(to),
        rlp_encode_bytes(value),
        rlp_encode_bytes(call_data),
        rlp_encode_list(&[]), // empty access list
        rlp_encode_uint(v),
        rlp_encode_bytes(&r_bytes),
        rlp_encode_bytes(&s_bytes),
    ]);
    let mut raw_tx = vec![0x02u8];
    raw_tx.extend_from_slice(&signed_rlp);
    Ok(format!("0x{}", bytes_to_hex(&raw_tx)))
}
