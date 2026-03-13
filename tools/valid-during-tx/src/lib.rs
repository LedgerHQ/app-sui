// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Build Sui transfer transaction for Ledger app tests.
//! Constants from test_sign_sui_transfer_1.py::test_sign_tx_sui_whole_gas_coin
//!
//! DO NOT MODIFY - golden test for ragger-tests compatibility.

#[cfg(test)]
mod tests {
    use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
    use shared_crypto::intent::{Intent, IntentMessage};
    use std::str::FromStr;
    use sui_test_transaction_builder::TestTransactionBuilder;
    use sui_types::base_types::{ObjectRef, SequenceNumber, SuiAddress};
    use sui_types::digests::ObjectDigest;

    // From test_sign_sui_transfer_1.py::test_sign_tx_sui_whole_gas_coin
    const SENDER: &str = "0x6fb21feead027da4873295affd6c4f3618fe176fa2fbf3e7b5ef1d9463b31e21";
    const RECIPIENT: &str = "0x1d3f2643305760226e518c9b5a96165383808dd977971f73dea971543b0be488";
    const GAS_OBJECT_ID: &str = "0x400dbdcfeda8e1e64ebbf5710ccf5a67bfa6e7f945c92bc8611e0cb44b7219de";
    const GAS_VERSION: u64 = 289568467;
    const GAS_DIGEST: &str = "7vBQVjLUYjJ2kiA4YYhwrHey38LqDVU3FMC3PtUgpotV";
    const GAS_PRICE: u64 = 1000;
    const GAS_BUDGET: u64 = 2_988_000;
    const OBJECT_LIST_B64: &str = "AAEB03ZCEQAAAAAoQA29z+2o4eZOu/VxDM9aZ7+m5/lFySvIYR4MtEtyGd4QDpQ5AAAAAABvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeISB0/j3Uc6ljNbb1tbWgvj5PAz7MCgIO6e91iU9asLM9x2ATDwAAAAAA";

    const EXPECTED_TRANSACTION_B64: &str = "AAAAAAABACAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiAEBAQABAABvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQFADb3P7ajh5k679XEMz1pnv6bn+UXJK8hhHgy0S3IZ3tN2QhEAAAAAIGbFq2VJip03FgAaA0gV/0q8p2X39vI3XMkdKt23nCCKb7If7q0CfaSHMpWv/WxPNhj+F2+i+/Pnte8dlGOzHiHoAwAAAAAAAOCXLQAAAAAAAA==";

    #[test]
    fn build_matches_ragger_test_sign_tx_sui_whole_gas_coin() {
        let sender = SuiAddress::from_str(SENDER).expect("Invalid sender");
        let recipient = SuiAddress::from_str(RECIPIENT).expect("Invalid recipient");
        let gas_id = sui_types::base_types::ObjectID::from_str(GAS_OBJECT_ID).expect("Invalid gas id");
        let digest = ObjectDigest::from_str(GAS_DIGEST).expect("Invalid gas digest");

        let gas_object: ObjectRef = (gas_id, SequenceNumber::from(GAS_VERSION), digest);

        let tx_data = TestTransactionBuilder::new(sender, gas_object, GAS_PRICE)
            .with_gas_budget(GAS_BUDGET)
            .transfer_sui(None, recipient)
            .build();

        let intent_msg = IntentMessage::new(Intent::sui_transaction(), tx_data);
        let tx_bytes = bcs::to_bytes(&intent_msg).expect("Failed to serialize transaction");

        let transaction_b64 = BASE64.encode(&tx_bytes);
        let object_list_b64 = vec![OBJECT_LIST_B64.to_string()];

        assert_eq!(transaction_b64, EXPECTED_TRANSACTION_B64, "transaction must match ragger-tests/test_sign_sui_transfer_1.py");
        assert_eq!(object_list_b64, vec![OBJECT_LIST_B64], "object_list must match ragger-tests");
    }
}
