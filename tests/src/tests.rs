use super::*;
use ckb_system_scripts::BUNDLED_CELL;
use ckb_testtool::ckb_crypto::secp::Generator;
use ckb_testtool::ckb_types::{bytes::Bytes, core::TransactionBuilder, packed::*, prelude::*};
use ckb_testtool::{builtin::ALWAYS_SUCCESS, context::Context};
use helper::*;
use molecule::prelude::*;

const MAX_CYCLES: u64 = 10_000_000;

#[test]
fn test_selection_success() {
    // deploy contract
    let mut context = Context::default();
    let contract_bin: Bytes = Loader::default().load_binary("selection");
    let out_point = context.deploy_cell(contract_bin);
    let always_success_out_point = context.deploy_cell(ALWAYS_SUCCESS.clone());

    // prepare lock_args
    let always_success_lock_script = context
        .build_script(&always_success_out_point, Bytes::new())
        .expect("always_success script");
    let always_success_script_dep = CellDep::new_builder()
        .out_point(always_success_out_point)
        .build();
    let omni_lock_hash = always_success_lock_script.calc_script_hash();
    let selection_args = axon::SelectionLockArgs::new_builder()
        .omni_lock_hash(axon_byte32(&omni_lock_hash))
        .checkpoint_lock_hash(axon_byte32(&Byte32::default()))
        .build();

    // prepare scripts
    let lock_script = context
        .build_script(&out_point, selection_args.as_bytes())
        .expect("selection script");
    let lock_script_dep = CellDep::new_builder().out_point(out_point).build();

    // prepare inputs and outputs
    let inputs = vec![
        // omni cell
        CellInput::new_builder()
            .previous_output(
                context.create_cell(
                    CellOutput::new_builder()
                        .capacity(500.pack())
                        .lock(always_success_lock_script.clone())
                        .build(),
                    Bytes::new(),
                ),
            )
            .build(),
        // selection cell
        CellInput::new_builder()
            .previous_output(
                context.create_cell(
                    CellOutput::new_builder()
                        .capacity(500.pack())
                        .lock(lock_script.clone())
                        .build(),
                    Bytes::new(),
                ),
            )
            .build(),
    ];
    let outputs = vec![
        // omni cell
        CellOutput::new_builder()
            .capacity(500u64.pack())
            .lock(always_success_lock_script)
            .build(),
        // selection cell
        CellOutput::new_builder()
            .capacity(500u64.pack())
            .lock(lock_script)
            .build(),
    ];

    // prepare outputs_data
    let outputs_data = vec![Bytes::new(), Bytes::new()];

    // build transaction
    let tx = TransactionBuilder::default()
        .inputs(inputs)
        .outputs(outputs)
        .outputs_data(outputs_data.pack())
        .cell_dep(lock_script_dep)
        .cell_dep(always_success_script_dep)
        .build();
    let tx = context.complete_tx(tx);

    // run
    let cycles = context
        .verify_tx(&tx, MAX_CYCLES)
        .expect("pass verification");
    println!("consume cycles: {}", cycles);
}

#[test]
fn test_checkpoint_success() {
    // init context
    let mut context = Context::default();
    let secp256k1_data_bin = BUNDLED_CELL.get("specs/cells/secp256k1_data").unwrap();
    let secp256k1_data_out_point = context.deploy_cell(secp256k1_data_bin.to_vec().into());
    let secp256k1_data_dep = CellDep::new_builder()
        .out_point(secp256k1_data_out_point)
        .build();
    let contract_bin: Bytes = Loader::default().load_binary("checkpoint");
    let contract_out_point = context.deploy_cell(contract_bin);
    let contract_dep = CellDep::new_builder()
        .out_point(contract_out_point.clone())
        .build();
    let always_success_out_point = context.deploy_cell(ALWAYS_SUCCESS.clone());
    let always_success_lock_script = context
        .build_script(&always_success_out_point, Bytes::from(vec![1]))
        .expect("always_success script");
    let type_id_type_script = context
        .build_script(&always_success_out_point, Bytes::new())
        .expect("type_id script");
    let at_type_script = context
        .build_script(&always_success_out_point, Bytes::from(vec![1]))
        .expect("at script");
    let always_success_script_dep = CellDep::new_builder()
        .out_point(always_success_out_point)
        .build();

    // prepare checkpoint_args and checkpoint_data
    let keypair = Generator::random_keypair();
    let checkpoint_args = axon::CheckpointLockArgs::new_builder()
        .admin_identity(axon_identity(&keypair.1))
        .type_id_hash(axon_byte32(&type_id_type_script.calc_script_hash()))
        .build();
    let checkpoint_data = axon_checkpoint_data(1, 1, &at_type_script.calc_script_hash());

    // prepare checkpoint lock_script
    let checkpoint_lock_script = context
        .build_script(&contract_out_point, checkpoint_args.as_bytes())
        .expect("checkpoint script");

    // prepare tx inputs and outputs
    let inputs = vec![
        // checkpoint cell
        CellInput::new_builder()
            .previous_output(
                context.create_cell(
                    CellOutput::new_builder()
                        .capacity(1000.pack())
                        .lock(checkpoint_lock_script.clone())
                        .type_(Some(type_id_type_script.clone()).pack())
                        .build(),
                    checkpoint_data.as_bytes(),
                ),
            )
            .build(),
        // AT cell 1
        CellInput::new_builder()
            .previous_output(
                context.create_cell(
                    CellOutput::new_builder()
                        .lock(always_success_lock_script.clone())
                        .type_(Some(at_type_script.clone()).pack())
                        .build(),
                    Bytes::from(2000u128.to_le_bytes().to_vec()),
                ),
            )
            .build(),
        // AT cell 2
        CellInput::new_builder()
            .previous_output(
                context.create_cell(
                    CellOutput::new_builder()
                        .capacity(3000.pack())
                        .lock(always_success_lock_script.clone())
                        .type_(Some(at_type_script.clone()).pack())
                        .build(),
                    Bytes::from(3000u128.to_le_bytes().to_vec()),
                ),
            )
            .build(),
    ];
    let outputs = vec![
        // checkpoint cell
        CellOutput::new_builder()
            .capacity(1000.pack())
            .lock(checkpoint_lock_script)
            .type_(Some(type_id_type_script).pack())
            .build(),
        // AT cell
        CellOutput::new_builder()
            .lock(always_success_lock_script)
            .type_(Some(at_type_script).pack())
            .build(),
    ];

    // prepare outputs_data
    let outputs_data = vec![
        checkpoint_data.as_bytes(),
        Bytes::from(5000u128.to_le_bytes().to_vec()),
    ];

    // prepare signed tx
    let tx = TransactionBuilder::default()
        .inputs(inputs)
        .outputs(outputs)
        .outputs_data(outputs_data.pack())
        .cell_dep(contract_dep)
        .cell_dep(always_success_script_dep)
        .cell_dep(secp256k1_data_dep)
        .build();
    let tx = context.complete_tx(tx);
    let tx = sign_tx(tx, &keypair.0, 0);

    // run
    let cycles = context
        .verify_tx(&tx, MAX_CYCLES)
        .expect("pass verification");
    println!("consume cycles: {}", cycles);
}

#[test]
fn test_withdrawal_success() {
    // init context
    let mut context = Context::default();
    let secp256k1_data_bin = BUNDLED_CELL.get("specs/cells/secp256k1_data").unwrap();
    let secp256k1_data_out_point = context.deploy_cell(secp256k1_data_bin.to_vec().into());
    let secp256k1_data_dep = CellDep::new_builder()
        .out_point(secp256k1_data_out_point)
        .build();
    let contract_bin: Bytes = Loader::default().load_binary("withdrawal");
    let contract_out_point = context.deploy_cell(contract_bin);
    let contract_dep = CellDep::new_builder()
        .out_point(contract_out_point.clone())
        .build();
    let always_success_out_point = context.deploy_cell(ALWAYS_SUCCESS.clone());
    let always_success_lock_script = context
        .build_script(&always_success_out_point, Bytes::from(vec![1]))
        .expect("always_success script");
    let type_id_type_script = context
        .build_script(&always_success_out_point, Bytes::new())
        .expect("type_id script");
    let at_type_script = context
        .build_script(&always_success_out_point, Bytes::from(vec![1]))
        .expect("at script");
    let always_success_script_dep = CellDep::new_builder()
        .out_point(always_success_out_point)
        .build();

    // prepare checkpoint_args and checkpoint_data
    let keypair = Generator::random_keypair();
    let withdrawal_args = axon::WithdrawalLockArgs::new_builder()
        .admin_identity(axon_identity(&keypair.1))
        .checkpoint_cell_type_hash(axon_byte32(&type_id_type_script.calc_script_hash()))
        .node_identity(axon_identity_opt(&keypair.1))
        .build();
    let withdrawal_data = axon_withdrawal_data(1);

    // prepare checkpoint lock_script
    let withdrawal_lock_script = context
        .build_script(&contract_out_point, withdrawal_args.as_bytes())
        .expect("withdrawal script");

    // prepare tx inputs and outputs
    let inputs = vec![
        // withdrawal cell
        CellInput::new_builder()
            .previous_output(
                context.create_cell(
                    CellOutput::new_builder()
                        .capacity(1000.pack())
                        .lock(withdrawal_lock_script.clone())
                        .type_(Some(at_type_script.clone()).pack())
                        .build(),
                    Bytes::from(withdrawal_data.clone()),
                ),
            )
            .build(),
    ];
    let outputs = vec![
        // withdrawal cell
        CellOutput::new_builder()
            .capacity(1000.pack())
            .lock(withdrawal_lock_script)
            .type_(Some(at_type_script).pack())
            .build(),
    ];

    // prepare outputs_data
    let outputs_data = vec![Bytes::from(withdrawal_data)];

    // prepare checkpoint cell_dep
    let checkpoint_data = axon_checkpoint_data(1, 1, &type_id_type_script.calc_script_hash());
    let checkpoint_script_dep = CellDep::new_builder()
        .out_point(
            context.create_cell(
                CellOutput::new_builder()
                    .capacity(1000.pack())
                    .lock(always_success_lock_script)
                    .type_(Some(type_id_type_script).pack())
                    .build(),
                checkpoint_data.as_bytes(),
            ),
        )
        .build();

    // prepare signed tx
    let tx = TransactionBuilder::default()
        .inputs(inputs)
        .outputs(outputs)
        .outputs_data(outputs_data.pack())
        .cell_dep(contract_dep)
        .cell_dep(always_success_script_dep)
        .cell_dep(secp256k1_data_dep)
        .cell_dep(checkpoint_script_dep)
        .build();
    let tx = context.complete_tx(tx);
    let tx = sign_tx(tx, &keypair.0, 1);

    // run
    let cycles = context
        .verify_tx(&tx, MAX_CYCLES)
        .expect("pass verification");
    println!("consume cycles: {}", cycles);
}

#[test]
fn test_stake_success() {
    // init context
    let mut context = Context::default();
    let secp256k1_data_bin = BUNDLED_CELL.get("specs/cells/secp256k1_data").unwrap();
    let secp256k1_data_out_point = context.deploy_cell(secp256k1_data_bin.to_vec().into());
    let secp256k1_data_dep = CellDep::new_builder()
        .out_point(secp256k1_data_out_point)
        .build();
    let contract_bin: Bytes = Loader::default().load_binary("stake");
    let contract_out_point = context.deploy_cell(contract_bin);
    let contract_dep = CellDep::new_builder()
        .out_point(contract_out_point.clone())
        .build();
    let always_success_out_point = context.deploy_cell(ALWAYS_SUCCESS.clone());
    let always_success_lock_script = context
        .build_script(&always_success_out_point, Bytes::from(vec![1]))
        .expect("always_success script");
    let type_id_type_script = context
        .build_script(&always_success_out_point, Bytes::new())
        .expect("type_id script");
    let always_success_script_dep = CellDep::new_builder()
        .out_point(always_success_out_point)
        .build();

    // prepare checkpoint_args and checkpoint_data
    let keypair = Generator::random_keypair();
    let stake_args = axon::StakeLockArgs::new_builder()
        .admin_identity(axon_identity(&keypair.1))
        .type_id_hash(axon_byte32(&type_id_type_script.calc_script_hash()))
        .node_identity(axon_identity_opt(&keypair.1))
        .build();
    let stake_data = axon_stake_data(70, &type_id_type_script.calc_script_hash(), vec![]);

    // prepare checkpoint lock_script
    let stake_lock_script = context
        .build_script(&contract_out_point, stake_args.as_bytes())
        .expect("stake script");

    // prepare tx inputs and outputs
    let inputs = vec![
        // stake cell
        CellInput::new_builder()
            .previous_output(
                context.create_cell(
                    CellOutput::new_builder()
                        .capacity(1000.pack())
                        .lock(stake_lock_script.clone())
                        .type_(Some(type_id_type_script.clone()).pack())
                        .build(),
                    stake_data.as_bytes(),
                ),
            )
            .build(),
    ];
    let outputs = vec![
        // withdrawal cell
        CellOutput::new_builder()
            .capacity(1000.pack())
            .lock(stake_lock_script)
            .type_(Some(type_id_type_script.clone()).pack())
            .build(),
    ];

    // prepare outputs_data
    let outputs_data = vec![stake_data.as_bytes()];

    // prepare checkpoint cell_dep
    let checkpoint_data = axon_checkpoint_data(1, 1, &type_id_type_script.calc_script_hash());
    let checkpoint_script_dep = CellDep::new_builder()
        .out_point(
            context.create_cell(
                CellOutput::new_builder()
                    .capacity(1000.pack())
                    .lock(always_success_lock_script)
                    .type_(Some(type_id_type_script).pack())
                    .build(),
                checkpoint_data.as_bytes(),
            ),
        )
        .build();

    // prepare signed tx
    let tx = TransactionBuilder::default()
        .inputs(inputs)
        .outputs(outputs)
        .outputs_data(outputs_data.pack())
        .cell_dep(contract_dep)
        .cell_dep(always_success_script_dep)
        .cell_dep(secp256k1_data_dep)
        .cell_dep(checkpoint_script_dep)
        .build();
    let tx = context.complete_tx(tx);
    let tx = sign_tx(tx, &keypair.0, 1);

    // run
    let cycles = context
        .verify_tx(&tx, MAX_CYCLES)
        .expect("pass verification");
    println!("consume cycles: {}", cycles);
}
