#![allow(dead_code)]

use crate::axon;
use blst::min_pk::SecretKey;
use ckb_testtool::ckb_crypto::secp::Privkey;
use ckb_testtool::ckb_hash::{blake2b_256, new_blake2b};
use ckb_testtool::ckb_types::{
    bytes::Bytes,
    core::{ScriptHashType, TransactionView},
    packed::{self, *},
    prelude::*,
    H256,
};
use ckb_vm::{
    machine::{
        asm::{AsmCoreMachine, AsmMachine},
        DefaultMachineBuilder, VERSION1,
    },
    ISA_IMC,
};
use molecule::prelude::*;
use rand::prelude::*;

pub fn blake160(data: &[u8]) -> [u8; 20] {
    let mut buf = [0u8; 20];
    let hash = blake2b_256(data);
    buf.clone_from_slice(&hash[..20]);
    buf
}

pub fn axon_byte48(bytes: &[u8; 48]) -> axon::Byte48 {
    axon::Byte48::new_unchecked(bytes.to_vec().into())
}

pub fn axon_byte32(bytes: &Byte32) -> axon::Byte32 {
    let bytes: [u8; 32] = bytes.unpack();
    axon::Byte32::new_unchecked(bytes.to_vec().into())
}

pub fn axon_byte20(bytes: &[u8; 20]) -> axon::Byte20 {
    axon::Byte20::new_unchecked(bytes.to_vec().into())
}

pub fn axon_byte16(value: u128) -> axon::Byte16 {
    axon::Byte16::new_unchecked(value.to_le_bytes().to_vec().into())
}

pub fn axon_byte8(value: u64) -> axon::Byte8 {
    axon::Byte8::new_unchecked(value.to_le_bytes().to_vec().into())
}

pub fn axon_byte4(value: u32) -> axon::Byte4 {
    axon::Byte4::new_unchecked(value.to_le_bytes().to_vec().into())
}

pub fn axon_bytes(bytes: &Vec<u8>) -> axon::Bytes {
    let bytes = bytes.into_iter().map(|value| (*value).into()).collect();
    axon::Bytes::new_builder().set(bytes).build()
}

pub fn axon_identity(pubkey: &Vec<u8>) -> axon::Identity {
    let pubkey_hash = blake160(pubkey.as_slice());
    axon::Identity::new_builder()
        .flag(Byte::from(0))
        .content(axon_byte20(&pubkey_hash))
        .build()
}

pub fn axon_identity_opt(pubkey: &Vec<u8>) -> axon::IdentityOpt {
    axon::IdentityOpt::new_builder()
        .set(Some(axon_identity(pubkey)))
        .build()
}

pub fn axon_identity_none() -> axon::IdentityOpt {
    axon::IdentityOpt::new_builder().set(None).build()
}

pub fn axon_checkpoint_data(
    sudt_type_hash: &Byte32,
    stake_type_hash: &Byte32,
    withdrawal_lock_code_hash: &Byte32,
) -> axon::CheckpointLockCellData {
    axon::CheckpointLockCellData::new_builder()
        .state(1u8.into())
        .era(axon_byte8(1))
        .base_reward(axon_byte16(0))
        .unlock_period(axon_byte4(1))
        .period_interval(axon_byte4(100))
        .era_period(axon_byte4(1))
        .period(axon_byte8(1))
        .half_period(axon_byte8(1))
        .sudt_type_hash(axon_byte32(sudt_type_hash))
        .stake_type_hash(axon_byte32(stake_type_hash))
        .block_hash(axon_byte32(&[0u8; 32].pack()))
        .withdrawal_lock_code_hash(axon_byte32(withdrawal_lock_code_hash))
        .build()
}

pub fn axon_withdrawal_data(sudt: u128, period: u64) -> Vec<u8> {
    let mut data = vec![];
    data.append(&mut sudt.to_le_bytes().to_vec());
    data.append(&mut period.to_le_bytes().to_vec());
    data
}

pub fn axon_stake_info(
    pubkey: &Vec<u8>,
    bls_pubkey: &[u8; 48],
    stake_amount: u128,
    era: u64,
) -> axon::StakeInfo {
    let pubkey_hash = blake160(pubkey);
    let identity = axon::Identity::new_builder()
        .flag(Byte::from(0))
        .content(axon_byte20(&pubkey_hash))
        .build();
    let mut l2_address = [0u8; 20];
    l2_address.copy_from_slice(&pubkey[..20]);
    axon::StakeInfo::new_builder()
        .identity(identity)
        .l2_address(axon_byte20(&l2_address))
        .bls_pub_key(axon_byte48(bls_pubkey))
        .stake_amount(axon_byte16(stake_amount))
        .inauguration_era(axon_byte8(era))
        .build()
}

pub fn axon_stake_data(
    quorum: u8,
    checkpoint_type_hash: &Byte32,
    sudt_type_hash: &Byte32,
    infos: &Vec<axon::StakeInfo>,
) -> axon::StakeLockCellData {
    let stake_infos = axon::StakeInfoVec::new_builder().set(infos.clone()).build();
    axon::StakeLockCellData::new_builder()
        .checkpoint_type_hash(axon_byte32(checkpoint_type_hash))
        .sudt_type_hash(axon_byte32(sudt_type_hash))
        .stake_infos(stake_infos)
        .quorum_size(quorum.into())
        .build()
}

pub fn axon_at_data(amount: u128, period: u64) -> [u8; 24] {
    let mut data = [0u8; 24];
    data[..16].copy_from_slice(&amount.to_le_bytes());
    data[16..].copy_from_slice(&period.to_le_bytes());
    data
}

pub fn sign_tx(tx: TransactionView, key: &Privkey, mode: u8) -> TransactionView {
    let mut signed_witnesses: Vec<packed::Bytes> = Vec::new();
    let mut blake2b = new_blake2b();
    blake2b.update(&tx.hash().raw_data());
    // digest the first witness
    let witness = WitnessArgs::new_builder()
        .lock(Some(Bytes::from(vec![0u8; 65])).pack())
        .input_type(Some(Bytes::from(vec![mode])).pack())
        .build();
    let witness_size = witness.as_bytes().len() as u64;
    let mut message = [0u8; 32];
    blake2b.update(&witness_size.to_le_bytes());
    blake2b.update(&witness.as_bytes());
    blake2b.finalize(&mut message);
    let message = H256::from(message);
    let sig = key.sign_recoverable(&message).expect("sign");
    signed_witnesses.push(
        witness
            .as_builder()
            .lock(Some(Bytes::from(sig.serialize())).pack())
            .build()
            .as_bytes()
            .pack(),
    );
    tx.as_advanced_builder()
        .set_witnesses(signed_witnesses)
        .build()
}

pub fn blst_sign_tx(tx: TransactionView, key: &SecretKey, mode: u8) -> TransactionView {
    let mut signed_witnesses: Vec<packed::Bytes> = Vec::new();
    let mut blake2b = new_blake2b();
    blake2b.update(&tx.hash().raw_data());
    // digest the first witness
    let witness = WitnessArgs::new_builder()
        .lock(Some(Bytes::from(vec![0u8; 144])).pack())
        .input_type(Some(Bytes::from(vec![mode])).pack())
        .build();
    let witness_size = witness.as_bytes().len() as u64;
    let mut message = [0u8; 32];
    blake2b.update(&witness_size.to_le_bytes());
    blake2b.update(&witness.as_bytes());
    blake2b.finalize(&mut message);
    let sig = key
        .sign(
            &message,
            b"BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_NUL_",
            &[],
        )
        .compress();
    let pubkey = key.sk_to_pk().compress();
    let compose = {
        let mut value = [0u8; 144];
        value[..48].copy_from_slice(&pubkey);
        value[48..].copy_from_slice(&sig);
        assert!(value != [0u8; 144]);
        value.to_vec()
    };
    signed_witnesses.push(
        witness
            .as_builder()
            .lock(Some(Bytes::from(compose)).pack())
            .build()
            .as_bytes()
            .pack(),
    );
    tx.as_advanced_builder()
        .set_witnesses(signed_witnesses)
        .build()
}

pub fn random_bls_keypair() -> (SecretKey, Vec<u8>) {
    let mut rng = thread_rng();
    let mut ikm = [0u8; 32];
    rng.fill_bytes(&mut ikm);
    let privkey = SecretKey::key_gen(&ikm, &[]).unwrap();
    let pubkey = privkey.sk_to_pk();
    (privkey, pubkey.compress().to_vec())
}

pub fn calc_withdrawal_lock_hash(
    withdrawal_code_hash: Byte32,
    admin_identity: axon::Identity,
    checkpoint_type_hash: axon::Byte32,
    node_identity: axon::IdentityOpt,
) -> Byte32 {
    let withdrawal_lock_args = axon::WithdrawalLockArgs::new_builder()
        .admin_identity(admin_identity)
        .checkpoint_cell_type_hash(checkpoint_type_hash)
        .node_identity(node_identity)
        .build();
    Script::new_builder()
        .code_hash(withdrawal_code_hash)
        .hash_type(ScriptHashType::Type.into())
        .args(withdrawal_lock_args.as_slice().pack())
        .build()
        .calc_script_hash()
}

pub fn run_ckb_vm(code: &Bytes, args: &Vec<Bytes>) -> i8 {
    let asm_core = AsmCoreMachine::new(ISA_IMC, VERSION1, u64::max_value());
    let core = DefaultMachineBuilder::new(asm_core).build();
    let mut machine = AsmMachine::new(core, None);
    machine.load_program(&code, &args).unwrap();
    machine.run().expect("run ckb-vm")
}
