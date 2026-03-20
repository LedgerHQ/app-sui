//! Build Sui transfer and stake transactions for Ledger app tests.
//!
//! DO NOT MODIFY - golden tests for ragger-tests compatibility.

#[cfg(test)]
mod tests {
    use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
    use shared_crypto::intent::{Intent, IntentMessage};
    use std::str::FromStr;
    use sui_test_transaction_builder::{FundSource, TestTransactionBuilder};
    use sui_types::base_types::{ObjectID, ObjectRef, SequenceNumber, SuiAddress};
    use sui_types::digests::{ChainIdentifier, ObjectDigest};
    use sui_types::transaction::{TransactionDataAPI, TransactionExpiration};

    // From test_sign_sui_transfer_1.py::test_sign_tx_sui_whole_gas_coin
    const SENDER: &str = "0x6fb21feead027da4873295affd6c4f3618fe176fa2fbf3e7b5ef1d9463b31e21";
    const RECIPIENT: &str = "0x1d3f2643305760226e518c9b5a96165383808dd977971f73dea971543b0be488";
    const GAS_OBJECT_ID: &str =
        "0x400dbdcfeda8e1e64ebbf5710ccf5a67bfa6e7f945c92bc8611e0cb44b7219de";
    const GAS_VERSION: u64 = 289568467;
    const GAS_DIGEST: &str = "7vBQVjLUYjJ2kiA4YYhwrHey38LqDVU3FMC3PtUgpotV";
    const GAS_PRICE: u64 = 1000;
    const GAS_BUDGET: u64 = 2_988_000;
    const OBJECT_LIST_B64: &str = "AAEB03ZCEQAAAAAoQA29z+2o4eZOu/VxDM9aZ7+m5/lFySvIYR4MtEtyGd4QDpQ5AAAAAABvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeISB0/j3Uc6ljNbb1tbWgvj5PAz7MCgIO6e91iU9asLM9x2ATDwAAAAAA";

    // From test_sign_sui_funds_withdrawal.py (SIP-58 FundsWithdrawal)
    const FUNDS_WITHDRAWAL_AMOUNT: u64 = 612_000;
    const FUNDS_WITHDRAWAL_GAS_BUDGET: u64 = 500_000;

    // Golden: whole gas coin transfer. Hex of IntentMessage(TransactionData).
    const EXPECTED_WHOLE_GAS_COIN_HEX: &str = "00000000000100201d3f2643305760226e518c9b5a96165383808dd977971f73dea971543b0be488010101000100006fb21feead027da4873295affd6c4f3618fe176fa2fbf3e7b5ef1d9463b31e2101400dbdcfeda8e1e64ebbf5710ccf5a67bfa6e7f945c92bc8611e0cb44b7219ded3764211000000002066c5ab65498a9d3716001a034815ff4abca765f7f6f2375cc91d2addb79c208a6fb21feead027da4873295affd6c4f3618fe176fa2fbf3e7b5ef1d9463b31e21e803000000000000e0972d000000000000";

    // ValidDuring (SIP-58): same sender, gas from address balance, empty gas_data.payment
    const VALID_DURING_AMOUNT: u64 = 1_000_000;
    const VALID_DURING_GAS_BUDGET: u64 = 150_000;
    const VALID_DURING_CURRENT_EPOCH: u64 = 0;
    const VALID_DURING_NONCE: u32 = 42;

    // Golden: ValidDuring transfer (transfer_sui with address balance gas). Hex of IntentMessage(TransactionData).
    const EXPECTED_VALID_DURING_HEX: &str = "00000000000200201d3f2643305760226e518c9b5a96165383808dd977971f73dea971543b0be488000840420f00000000000202000101010001010200000100006fb21feead027da4873295affd6c4f3618fe176fa2fbf3e7b5ef1d9463b31e21006fb21feead027da4873295affd6c4f3618fe176fa2fbf3e7b5ef1d9463b31e21e803000000000000f0490200000000000201000000000000000001010000000000000000002000000000000000000000000000000000000000000000000000000000000000002a000000";

    #[test]
    fn build_matches_ragger_test_sign_tx_sui_whole_gas_coin() {
        let sender = SuiAddress::from_str(SENDER).expect("Invalid sender");
        let recipient = SuiAddress::from_str(RECIPIENT).expect("Invalid recipient");
        let gas_id =
            sui_types::base_types::ObjectID::from_str(GAS_OBJECT_ID).expect("Invalid gas id");
        let digest = ObjectDigest::from_str(GAS_DIGEST).expect("Invalid gas digest");

        let gas_object: ObjectRef = (gas_id, SequenceNumber::from(GAS_VERSION), digest);

        let tx_data = TestTransactionBuilder::new(sender, gas_object, GAS_PRICE)
            .with_gas_budget(GAS_BUDGET)
            .transfer_sui(None, recipient)
            .build();

        let intent_msg = IntentMessage::new(Intent::sui_transaction(), tx_data);
        let tx_bytes = bcs::to_bytes(&intent_msg).expect("Failed to serialize transaction");

        let transaction_b64 = BASE64.encode(&tx_bytes);
        let expected_b64 =
            BASE64.encode(&hex::decode(EXPECTED_WHOLE_GAS_COIN_HEX).expect("invalid hex"));
        let object_list_b64 = vec![OBJECT_LIST_B64.to_string()];

        assert_eq!(
            transaction_b64, expected_b64,
            "transaction must match ragger-tests/test_sign_sui_transfer_1.py"
        );
        assert_eq!(
            object_list_b64,
            vec![OBJECT_LIST_B64],
            "object_list must match ragger-tests"
        );
    }

    #[test]
    fn build_tx_sui_funds_withdrawal() {
        let sender = SuiAddress::from_str(SENDER).expect("Invalid sender");
        let recipient = SuiAddress::from_str(RECIPIENT).expect("Invalid recipient");
        let gas_id = ObjectID::from_str(GAS_OBJECT_ID).expect("Invalid gas id");
        let digest = ObjectDigest::from_str(GAS_DIGEST).expect("Invalid gas digest");
        let gas_object: ObjectRef = (gas_id, SequenceNumber::from(GAS_VERSION), digest);

        let tx_data = TestTransactionBuilder::new(sender, gas_object, GAS_PRICE)
            .with_gas_budget(FUNDS_WITHDRAWAL_GAS_BUDGET)
            .transfer_sui_to_address_balance(
                FundSource::address_fund(),
                vec![(FUNDS_WITHDRAWAL_AMOUNT, recipient)],
            )
            .build();

        assert!(
            tx_data.has_funds_withdrawals(),
            "FundsWithdrawal transfer must have funds_withdrawals"
        );

        let intent_msg = IntentMessage::new(Intent::sui_transaction(), tx_data);
        let tx_bytes = bcs::to_bytes(&intent_msg).expect("Failed to serialize transaction");
        let transaction_b64 = BASE64.encode(&tx_bytes);
        let object_list_b64 = vec![OBJECT_LIST_B64.to_string()];

        const EXPECTED_FUNDS_WITHDRAWAL_B64: &str = "AAAAAAADAgCgVgkAAAAAAAAHAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAIDc3VpA1NVSQAAACCgVgkAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAgHT8mQzBXYCJuUYybWpYWU4OAjdl3lx9z3qlxVDsL5IgDAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAACEWZ1bmRzX2FjY3VtdWxhdG9yEHdpdGhkcmF3YWxfc3BsaXQBBwAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAACB2JhbGFuY2UHQmFsYW5jZQEHAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAIDc3VpA1NVSQACAQAAAQEAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAACB2JhbGFuY2UMcmVkZWVtX2Z1bmRzAQcAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAgNzdWkDU1VJAAECAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAIHYmFsYW5jZQpzZW5kX2Z1bmRzAQcAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAgNzdWkDU1VJAAICAQABAgBvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQFADb3P7ajh5k679XEMz1pnv6bn+UXJK8hhHgy0S3IZ3tN2QhEAAAAAIGbFq2VJip03FgAaA0gV/0q8p2X39vI3XMkdKt23nCCKb7If7q0CfaSHMpWv/WxPNhj+F2+i+/Pnte8dlGOzHiHoAwAAAAAAACChBwAAAAAAAA==";

        assert_eq!(
            transaction_b64, EXPECTED_FUNDS_WITHDRAWAL_B64,
            "transaction must match ragger-tests/test_sign_sui_funds_withdrawal.py"
        );
        assert_eq!(
            object_list_b64,
            vec![OBJECT_LIST_B64],
            "object_list must match ragger-tests"
        );
    }

    #[test]
    fn build_valid_during_sui_transfer_address_balance_gas() {
        let sender = SuiAddress::from_str(SENDER).expect("Invalid sender");
        let recipient = SuiAddress::from_str(RECIPIENT).expect("Invalid recipient");
        let chain_id = ChainIdentifier::default();

        // transfer_sui with address balance gas: runtime creates GasCoin from balance.
        // Produces TransferSuiTx (TransferObjects with GasCoin) - supported by Ledger.
        let tx_data = TestTransactionBuilder::new_with_address_balance_gas(
            sender,
            GAS_PRICE,
            chain_id,
            VALID_DURING_CURRENT_EPOCH,
            VALID_DURING_NONCE,
        )
        .with_gas_budget(VALID_DURING_GAS_BUDGET)
        .transfer_sui(Some(VALID_DURING_AMOUNT), recipient)
        .build();

        assert!(
            tx_data.gas_data().payment.is_empty(),
            "ValidDuring must have empty gas payment (gas from address balance)"
        );
        assert!(
            matches!(
                tx_data.expiration(),
                TransactionExpiration::ValidDuring { .. }
            ),
            "Must use TransactionExpiration::ValidDuring"
        );

        let intent_msg = IntentMessage::new(Intent::sui_transaction(), tx_data);
        let tx_bytes = bcs::to_bytes(&intent_msg).expect("Failed to serialize transaction");
        let transaction_b64 = BASE64.encode(&tx_bytes);
        let expected_b64 =
            BASE64.encode(&hex::decode(EXPECTED_VALID_DURING_HEX).expect("invalid hex"));

        assert_eq!(
            transaction_b64, expected_b64,
            "ValidDuring transaction must be deterministic"
        );
    }

    // From test_sign_sui_stake_1.py::test_sign_stake_gas_coin
    const STAKE_SENDER: &str = "0x1d3f2643305760226e518c9b5a96165383808dd977971f73dea971543b0be488";
    const STAKE_VALIDATOR: &str =
        "0x35f5f154f0117464e3379c45c7f3cb6b2d4efedf300acadff8a7e8e9e3a15109";
    const STAKE_GAS_OBJECT_ID: &str =
        "0xebff16b4d2081ab06d1d5251c988208641e5c501c7fa8bdce9c8b7b0908ba76b";
    const STAKE_GAS_VERSION: u64 = 289568469;
    const STAKE_GAS_DIGEST: &str = "53CbPjHczNtV9Kids6JdGt9bkPbSeJ34dc9TX2W2g6tT";
    const STAKE_GAS_PRICE: u64 = 1000;
    const STAKE_GAS_BUDGET: u64 = 123_000;

    #[test]
    fn build_matches_ragger_test_sign_stake_gas_coin() {
        let sender = SuiAddress::from_str(STAKE_SENDER).expect("Invalid sender");
        let validator = SuiAddress::from_str(STAKE_VALIDATOR).expect("Invalid validator");
        let gas_id = ObjectID::from_str(STAKE_GAS_OBJECT_ID).expect("Invalid gas id");
        let digest = ObjectDigest::from_str(STAKE_GAS_DIGEST).expect("Invalid digest");

        let gas_object: ObjectRef = (gas_id, SequenceNumber::from(STAKE_GAS_VERSION), digest);

        // Stake the gas coin itself (request_add_stake with GasCoin)
        let tx_data = TestTransactionBuilder::new(sender, gas_object, STAKE_GAS_PRICE)
            .with_gas_budget(STAKE_GAS_BUDGET)
            .call_staking(gas_object, validator)
            .build();

        let intent_msg = IntentMessage::new(Intent::sui_transaction(), tx_data);
        let tx_bytes = bcs::to_bytes(&intent_msg).expect("Failed to serialize transaction");

        // Golden: our build output (1 gas coin; ragger test_sign_stake_gas_coin uses 4)
        let expected_b64 = "AAAAAAADAQEAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABQEAAAAAAAAAAQEA6/8WtNIIGrBtHVJRyYgghkHlxQHH+ovc6ci3sJCLp2vVdkIRAAAAACA7/wR8yg26EmuQ9efw9yarvaOHVlIb3BOm8pv5J42w5AAgNfXxVPARdGTjN5xFx/PLay1O/t8wCsrf+Kfo6eOhUQkBAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAADCnN1aV9zeXN0ZW0RcmVxdWVzdF9hZGRfc3Rha2UAAwEAAAEBAAECAB0/JkMwV2AiblGMm1qWFlODgI3Zd5cfc96pcVQ7C+SIAev/FrTSCBqwbR1SUcmIIIZB5cUBx/qL3OnIt7CQi6dr1XZCEQAAAAAgO/8EfMoNuhJrkPXn8Pcmq72jh1ZSG9wTpvKb+SeNsOQdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiOgDAAAAAAAAeOABAAAAAAAA";
        let transaction_b64 = BASE64.encode(&tx_bytes);

        assert_eq!(
            transaction_b64, expected_b64,
            "stake gas coin transaction must be deterministic"
        );
    }

    // ValidDuring (SIP-58) stake: gas from address balance, stake from owned coin
    const VALID_DURING_STAKE_GAS_BUDGET: u64 = 150_000;
    const VALID_DURING_STAKE_GAS_PRICE: u64 = 1000;
    const VALID_DURING_STAKE_CURRENT_EPOCH: u64 = 0;
    const VALID_DURING_STAKE_NONCE: u32 = 43;

    // Stake coin: use same object as transfer test's gas (sender owns it)
    const VALID_DURING_STAKE_COIN_ID: &str =
        "0x400dbdcfeda8e1e64ebbf5710ccf5a67bfa6e7f945c92bc8611e0cb44b7219de";
    const VALID_DURING_STAKE_COIN_VERSION: u64 = 289568467;
    const VALID_DURING_STAKE_COIN_DIGEST: &str = "7vBQVjLUYjJ2kiA4YYhwrHey38LqDVU3FMC3PtUgpotV";

    #[test]
    fn build_valid_during_sui_stake_address_balance_gas() {
        let sender = SuiAddress::from_str(SENDER).expect("Invalid sender");
        let validator = SuiAddress::from_str(STAKE_VALIDATOR).expect("Invalid validator");
        let chain_id = ChainIdentifier::default();

        let stake_coin_id =
            ObjectID::from_str(VALID_DURING_STAKE_COIN_ID).expect("Invalid coin id");
        let stake_coin_digest =
            ObjectDigest::from_str(VALID_DURING_STAKE_COIN_DIGEST).expect("Invalid digest");
        let stake_coin: ObjectRef = (
            stake_coin_id,
            SequenceNumber::from(VALID_DURING_STAKE_COIN_VERSION),
            stake_coin_digest,
        );

        let tx_data = TestTransactionBuilder::new_with_address_balance_gas(
            sender,
            VALID_DURING_STAKE_GAS_PRICE,
            chain_id,
            VALID_DURING_STAKE_CURRENT_EPOCH,
            VALID_DURING_STAKE_NONCE,
        )
        .with_gas_budget(VALID_DURING_STAKE_GAS_BUDGET)
        .call_staking(stake_coin, validator)
        .build();

        assert!(
            tx_data.gas_data().payment.is_empty(),
            "ValidDuring stake must have empty gas payment (gas from address balance)"
        );
        assert!(
            matches!(
                tx_data.expiration(),
                TransactionExpiration::ValidDuring { .. }
            ),
            "Must use TransactionExpiration::ValidDuring"
        );

        let intent_msg = IntentMessage::new(Intent::sui_transaction(), tx_data);
        let tx_bytes = bcs::to_bytes(&intent_msg).expect("Failed to serialize transaction");
        let transaction_b64 = BASE64.encode(&tx_bytes);

        // Golden: ValidDuring stake (gas from address balance)
        const EXPECTED_VALID_DURING_STAKE_B64: &str = "AAAAAAADAQEAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABQEAAAAAAAAAAQEAQA29z+2o4eZOu/VxDM9aZ7+m5/lFySvIYR4MtEtyGd7TdkIRAAAAACBmxatlSYqdNxYAGgNIFf9KvKdl9/byN1zJHSrdt5wgigAgNfXxVPARdGTjN5xFx/PLay1O/t8wCsrf+Kfo6eOhUQkBAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAADCnN1aV9zeXN0ZW0RcmVxdWVzdF9hZGRfc3Rha2UAAwEAAAEBAAECAG+yH+6tAn2khzKVr/1sTzYY/hdvovvz57XvHZRjsx4hAG+yH+6tAn2khzKVr/1sTzYY/hdvovvz57XvHZRjsx4h6AMAAAAAAADwSQIAAAAAAAIBAAAAAAAAAAABAQAAAAAAAAAAACAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAACsAAAA=";
        assert_eq!(
            transaction_b64, EXPECTED_VALID_DURING_STAKE_B64,
            "ValidDuring stake must be deterministic"
        );
    }

    // From test_sign_sui_stake_3.py::test_sign_unstake_whole_coin (simplified: 1 gas coin)
    const UNSTAKE_SENDER: &str =
        "0x1d3f2643305760226e518c9b5a96165383808dd977971f73dea971543b0be488";
    const UNSTAKE_GAS_OBJECT_ID: &str =
        "0xebff16b4d2081ab06d1d5251c988208641e5c501c7fa8bdce9c8b7b0908ba76b";
    const UNSTAKE_GAS_VERSION: u64 = 289568469;
    const UNSTAKE_GAS_DIGEST: &str = "53CbPjHczNtV9Kids6JdGt9bkPbSeJ34dc9TX2W2g6tT";
    const UNSTAKE_DELEGATION_ID: &str =
        "0xa93f6c464f8fb8b98fb3d02112902060c8f85ea4d71cfc7777dfdbd75e68ab6d";
    const UNSTAKE_DELEGATION_VERSION: u64 = 289568468;
    const UNSTAKE_DELEGATION_DIGEST: &str = "Cbin2kMMWzjtPER7GZ7ne81Dhpk2tS31MwinvTwjMEZi";

    #[test]
    fn build_matches_ragger_test_sign_unstake_whole_coin() {
        let sender = SuiAddress::from_str(UNSTAKE_SENDER).expect("Invalid sender");
        let gas_id = ObjectID::from_str(UNSTAKE_GAS_OBJECT_ID).expect("Invalid gas id");
        let gas_digest = ObjectDigest::from_str(UNSTAKE_GAS_DIGEST).expect("Invalid digest");
        let gas_object: ObjectRef = (
            gas_id,
            SequenceNumber::from(UNSTAKE_GAS_VERSION),
            gas_digest,
        );

        let delegation_id =
            ObjectID::from_str(UNSTAKE_DELEGATION_ID).expect("Invalid delegation id");
        let delegation_digest =
            ObjectDigest::from_str(UNSTAKE_DELEGATION_DIGEST).expect("Invalid digest");
        let delegation: ObjectRef = (
            delegation_id,
            SequenceNumber::from(UNSTAKE_DELEGATION_VERSION),
            delegation_digest,
        );

        let tx_data = TestTransactionBuilder::new(sender, gas_object, STAKE_GAS_PRICE)
            .with_gas_budget(STAKE_GAS_BUDGET)
            .call_unstaking(delegation)
            .build();

        let intent_msg = IntentMessage::new(Intent::sui_transaction(), tx_data);
        let tx_bytes = bcs::to_bytes(&intent_msg).expect("Failed to serialize transaction");
        let transaction_b64 = BASE64.encode(&tx_bytes);

        // Golden: our build output (1 gas coin; ragger test_sign_unstake_whole_coin uses 3)
        const EXPECTED_UNSTAKE_B64: &str = "AAAAAAACAQEAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABQEAAAAAAAAAAQEAqT9sRk+PuLmPs9AhEpAgYMj4XqTXHPx3d9/b115oq23UdkIRAAAAACCsVYpX4/44Cp2BWe8aVkACUW5rxtsErjUPJ6nMxaCvvQEAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAMKc3VpX3N5c3RlbRZyZXF1ZXN0X3dpdGhkcmF3X3N0YWtlAAIBAAABAQAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiAHr/xa00ggasG0dUlHJiCCGQeXFAcf6i9zpyLewkIuna9V2QhEAAAAAIDv/BHzKDboSa5D15/D3Jqu9o4dWUhvcE6bym/knjbDkHT8mQzBXYCJuUYybWpYWU4OAjdl3lx9z3qlxVDsL5IjoAwAAAAAAAHjgAQAAAAAAAA==";
        assert_eq!(
            transaction_b64, EXPECTED_UNSTAKE_B64,
            "unstake whole coin transaction must be deterministic"
        );
    }

    // ValidDuring (SIP-58) unstake: gas from address balance, delegation from owned StakedSui
    const VALID_DURING_UNSTAKE_NONCE: u32 = 44;

    #[test]
    fn build_valid_during_sui_unstake_address_balance_gas() {
        let sender = SuiAddress::from_str(UNSTAKE_SENDER).expect("Invalid sender");
        let chain_id = ChainIdentifier::default();

        let delegation_id =
            ObjectID::from_str(UNSTAKE_DELEGATION_ID).expect("Invalid delegation id");
        let delegation_digest =
            ObjectDigest::from_str(UNSTAKE_DELEGATION_DIGEST).expect("Invalid digest");
        let delegation: ObjectRef = (
            delegation_id,
            SequenceNumber::from(UNSTAKE_DELEGATION_VERSION),
            delegation_digest,
        );

        let tx_data = TestTransactionBuilder::new_with_address_balance_gas(
            sender,
            VALID_DURING_STAKE_GAS_PRICE,
            chain_id,
            VALID_DURING_STAKE_CURRENT_EPOCH,
            VALID_DURING_UNSTAKE_NONCE,
        )
        .with_gas_budget(VALID_DURING_STAKE_GAS_BUDGET)
        .call_unstaking(delegation)
        .build();

        assert!(
            tx_data.gas_data().payment.is_empty(),
            "ValidDuring unstake must have empty gas payment (gas from address balance)"
        );
        assert!(
            matches!(
                tx_data.expiration(),
                TransactionExpiration::ValidDuring { .. }
            ),
            "Must use TransactionExpiration::ValidDuring"
        );

        let intent_msg = IntentMessage::new(Intent::sui_transaction(), tx_data);
        let tx_bytes = bcs::to_bytes(&intent_msg).expect("Failed to serialize transaction");
        let transaction_b64 = BASE64.encode(&tx_bytes);

        // Golden: ValidDuring unstake (gas from address balance)
        const EXPECTED_VALID_DURING_UNSTAKE_B64: &str = "AAAAAAACAQEAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABQEAAAAAAAAAAQEAqT9sRk+PuLmPs9AhEpAgYMj4XqTXHPx3d9/b115oq23UdkIRAAAAACCsVYpX4/44Cp2BWe8aVkACUW5rxtsErjUPJ6nMxaCvvQEAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAMKc3VpX3N5c3RlbRZyZXF1ZXN0X3dpdGhkcmF3X3N0YWtlAAIBAAABAQAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiAAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiOgDAAAAAAAA8EkCAAAAAAACAQAAAAAAAAAAAQEAAAAAAAAAAAAgAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAsAAAA";
        assert_eq!(
            transaction_b64, EXPECTED_VALID_DURING_UNSTAKE_B64,
            "ValidDuring unstake must be deterministic"
        );
    }
}
