// Import from `core` instead of from `std` since we are in no-std mode
use core::result::Result;

// Import heap related library from `alloc`
// https://doc.rust-lang.org/alloc/index.html
use alloc::vec::Vec;

// Import CKB syscalls and structures
// https://nervosnetwork.github.io/ckb-std/riscv64imac-unknown-none-elf/doc/ckb_std/index.html
use ckb_std::{
    ckb_constants::Source,
    ckb_types::{bytes::Bytes, prelude::*},
    debug,
    high_level::{
        load_cell_capacity, load_cell_data, load_cell_type_hash, load_script, load_witness_args,
        QueryIter,
    },
};

use crate::error::Error;
use protocol::{axon, Cursor};

fn get_info_by_type_hash(
    type_hash: &Vec<u8>,
    source: Source,
) -> Result<(u64, axon::CheckpointLockCellData), Error> {
    let mut capacity = 0u64;
    let mut celldata = None;
    QueryIter::new(load_cell_type_hash, source)
        .enumerate()
        .map(|(i, cell_type_hash)| {
            if cell_type_hash.unwrap_or([0u8; 32]) != type_hash[..] {
                return Ok(());
            }
            if celldata.is_some() {
                return Err(Error::CheckpointCellError);
            }
            match load_cell_capacity(i, source) {
                Ok(value) => capacity = value,
                Err(err) => return Err(Error::from(err)),
            }
            match load_cell_data(i, source) {
                Ok(value) => {
                    celldata = Some(axon::CheckpointLockCellData::from(Cursor::from(value)))
                }
                Err(err) => return Err(Error::from(err)),
            }
            Ok(())
        })
        .collect::<Result<Vec<_>, _>>()?;
    if celldata.is_none() {
        return Err(Error::CheckpointCellError);
    }
    Ok((capacity, celldata.unwrap()))
}

fn get_sudt_by_type_hash(type_hash: &Vec<u8>, source: Source) -> Result<u128, Error> {
    let mut sudt = 0u128;
    QueryIter::new(load_cell_type_hash, source)
        .enumerate()
        .map(|(i, cell_type_hash)| {
            if cell_type_hash.unwrap_or([0u8; 32]) == type_hash[..] {
                match load_cell_data(i, source) {
                    Ok(value) => {
                        // check uint128_t format
                        if value.len() < 16 {
                            return Err(Error::BadSudtDataFormat);
                        }
                        sudt += bytes_to_u128(&value[..16].to_vec());
                    }
                    Err(err) => return Err(Error::from(err)),
                }
            }
            Ok(())
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(sudt)
}

fn bytes_to_u128(bytes: &Vec<u8>) -> u128 {
    let mut array: [u8; 16] = [0u8; 16];
    array.copy_from_slice(bytes.as_slice());
    u128::from_le_bytes(array)
}

fn bytes_to_u64(bytes: &Vec<u8>) -> u64 {
    let mut array: [u8; 8] = [0u8; 8];
    array.copy_from_slice(bytes.as_slice());
    u64::from_le_bytes(array)
}

pub fn main() -> Result<(), Error> {
    let script = load_script()?;
    let args: Bytes = script.args().unpack();

    let checkpoint_args: axon::CheckpointLockArgs = Cursor::from(args.to_vec()).into();
    let admin_identity = checkpoint_args.admin_identity();
    let type_id_hash = checkpoint_args.type_id_hash();

    // check input and output capacity and data from checkpoint cells
    let (input_checkpoint_capacity, input_checkpoint_data) =
        get_info_by_type_hash(&type_id_hash, Source::Input)?;
    let (output_checkpoint_capacity, output_checkpoint_data) =
        get_info_by_type_hash(&type_id_hash, Source::Output)?;
    if input_checkpoint_capacity != output_checkpoint_capacity {
        return Err(Error::CheckpointCapacityMismatch);
    }
    if input_checkpoint_data.version() != output_checkpoint_data.version()
        || input_checkpoint_data.period_interval() != output_checkpoint_data.period_interval()
        || input_checkpoint_data.era_period() != output_checkpoint_data.era_period()
        || input_checkpoint_data.base_reward() != output_checkpoint_data.base_reward()
        || input_checkpoint_data.half_period() != output_checkpoint_data.half_period()
        || input_checkpoint_data.sudt_type_hash() != output_checkpoint_data.sudt_type_hash()
        || input_checkpoint_data.stake_type_hash() != output_checkpoint_data.stake_type_hash()
        || input_checkpoint_data.withdrawal_lock_code_hash()
            != output_checkpoint_data.withdrawal_lock_code_hash()
    {
        return Err(Error::CheckpointDataMismatch);
    }

    // check this is wether admin mode or checkpoint mode
    let witness_args = load_witness_args(0, Source::GroupInput)?;
    let is_admin_mode = {
        let input_type = witness_args.input_type().to_opt();
        if input_type.is_none() {
            return Err(Error::BadWitnessInputType);
        }
        match input_type.unwrap().raw_data().to_vec().first() {
            Some(value) => value == &0,
            None => return Err(Error::BadWitnessInputType),
        }
    };

    // get AT coins from AT cell
    let sudt_type_hash = input_checkpoint_data.sudt_type_hash();
    let input_at_amount = get_sudt_by_type_hash(&sudt_type_hash, Source::Input)?;
    let output_at_amount = get_sudt_by_type_hash(&sudt_type_hash, Source::Output)?;

    debug!(
        "input_at_amount = {}, output_at_amount = {}",
        input_at_amount, output_at_amount
    );

    // admin mode
    if is_admin_mode {
        // check admin signature
        if !secp256k1::verify_signature(&mut admin_identity.content()) {
            return Err(Error::SignatureMismatch);
        }
        // check AT amount
        if input_at_amount < output_at_amount {
            return Err(Error::ATAmountMismatch);
        }
    // checkpoint mode
    } else {
        let checkpoint = witness_args.lock().to_opt();
        if checkpoint.is_none() {
            return Err(Error::WitnessLockError);
        }
        if input_checkpoint_data.state() != output_checkpoint_data.state()
            || input_checkpoint_data.unlock_period() != output_checkpoint_data.unlock_period()
        {
            return Err(Error::CheckpointDataMismatch);
        }

        // ?????? witness ?????? checkpoint??????????????????????????? checkpoint???rlp ?????????????????? L2_block_hash, L2_block_number, L2_signature,
        // L2_bitmap?????????????????????????????????????????????, L2_last_checkpoint_block_hash, L2_proposer ?????????????????? L2 ??? block_hash ????????????
        // ?????? block_hash???Kaccak ???????????????????????????????????? L2_block_hash???

        // ?????? stake_type_hash ??? cell_deps ????????? Stake Cell???????????????????????? output.era ?????????????????????????????? L2_bitmap ??????????????????
        // ?????????????????? 2/3 ???????????????????????? L2_bitmap ?????????????????????????????????????????? bls_puk_key????????? BLS ???????????????????????????

        // ?????? input.state == 0x01 && output.period == input.period + 1 && output.era == ???output.period/era_period??? && output.block_hash
        // == L2_block_hash && output.period * period_interval == L2_block_number && input.block_hash == L2_last_checkpoint_block_hash

        // check AT amount
        let base_reward = bytes_to_u128(&input_checkpoint_data.base_reward());
        let period = bytes_to_u64(&input_checkpoint_data.period());
        let half_period = bytes_to_u64(&input_checkpoint_data.half_period());
        if half_period == 0 {
            return Err(Error::CheckpointDataError);
        }
        if output_at_amount - input_at_amount
            != base_reward / 2u128.pow((period / half_period) as u32)
        {
            return Err(Error::ATAmountMismatch);
        }

        // construct Withdrawal lock

        // ?????? L2_proposer ??????????????????????????????????????? Identity???????????? admin_identity, stake_lock_hash ??? withdrawal_lock_code_hash
        // ??? withdrawal_lock_hash_type ????????? withdrawal lock?????????????????? withdrawal_lock_hash

        // ?????? withdrawal_lock_hash ??? sudt_type_hash ?????? input ??? output ??? Withdrawal AT cell????????? output ?????? - input ?????? ==
        // base_reward / (2^???period/half_period???)?????? output.{each Withdrawal AT cell}.period == output.{Checkpoint Cell}.period +
        // output.{Checkpoint Cell}.unlock_period
    }

    Ok(())
}
