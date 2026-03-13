//! Build Sui transfer transactions for Ledger app tests.
//!
//! DO NOT MODIFY - golden tests for ragger-tests compatibility.

#[cfg(test)]
mod tests {
    use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
    use shared_crypto::intent::{Intent, IntentMessage};
    use std::str::FromStr;
    use sui_test_transaction_builder::TestTransactionBuilder;
    use sui_types::base_types::{ObjectRef, SequenceNumber, SuiAddress};
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
}
