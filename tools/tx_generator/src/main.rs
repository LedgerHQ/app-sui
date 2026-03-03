use shared_crypto::intent::{Intent, IntentMessage};
use sui_sdk::types::{
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{Argument, Command, TransactionData},
};

use sui_types::{
    base_types::{ObjectID, ObjectRef, SuiAddress, SequenceNumber},
    digests::{ObjectDigest, TransactionDigest},
    object::{MoveObject, Object, Owner},
};

use base64::prelude::Engine as _;
use std::str::FromStr;

const SENDER: &str = "0x6fb21feead027da4873295affd6c4f3618fe176fa2fbf3e7b5ef1d9463b31e21";

/// Helper to decode a base58-encoded digest into a 32-byte array
fn decode_b58_digest(s: &str) -> [u8; 32] {
    bs58::decode(s)
        .into_vec()
        .unwrap()
        .try_into()
        .unwrap()
}

fn main() {
    // =========================================================================
    // 1) Build the transaction
    // =========================================================================
    let mut ptb = ProgrammableTransactionBuilder::new();

    let v = base64::prelude::BASE64_STANDARD
        .decode("HT8mQzBXYCJuUYybWpYWU4OAjdl3lx9z3qlxVDsL5Ig=")
        .unwrap();
    let s: String = v.iter().map(|b| format!("{:02X}", b)).collect();
    let recipient = SuiAddress::from_str(&s).unwrap();

    let argument_address = ptb.pure(recipient).unwrap();
    ptb.command(Command::TransferObjects(
        vec![Argument::GasCoin],
        argument_address,
    ));
    let builder = ptb.finish();

    let gas_budget = 2988000u64;
    let gas_price = 1000u64;
    let sender = SuiAddress::from_str(SENDER).unwrap();

    let gas_object_id = ObjectID::from_hex_literal(
        "0x400dbdcfeda8e1e64ebbf5710ccf5a67bfa6e7f945c92bc8611e0cb44b7219de",
    )
    .unwrap();
    let gas_object_version = SequenceNumber::from_u64(289568467);
    let gas_object_digest =
        ObjectDigest::new(decode_b58_digest("7vBQVjLUYjJ2kiA4YYhwrHey38LqDVU3FMC3PtUgpotV"));

    let tx_data = TransactionData::new_programmable(
        sender,
        vec![(gas_object_id, gas_object_version, gas_object_digest)],
        builder,
        gas_budget,
        gas_price,
    );

    let tx = IntentMessage::new(Intent::sui_transaction(), &tx_data);
    let base64_tx = base64::prelude::BASE64_STANDARD.encode(bcs::to_bytes(&tx).unwrap());
    println!("Transaction base64: {}", base64_tx);

    let ref_tx = "AAAAAAABACAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiAEBAQABAABvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQFADb3P7ajh5k679XEMz1pnv6bn+UXJK8hhHgy0S3IZ3tN2QhEAAAAAIGbFq2VJip03FgAaA0gV/0q8p2X39vI3XMkdKt23nCCKb7If7q0CfaSHMpWv/WxPNhj+F2+i+/Pnte8dlGOzHiHoAwAAAAAAAOCXLQAAAAAAAA==";
    assert_eq!(ref_tx, base64_tx, "Transaction mismatch!");
    println!("Transaction matches reference ✓");

    // =========================================================================
    // 2) Build the gas coin Object (ObjectInner) for the object list
    // =========================================================================
    // MoveObject::new_gas_coin(version, id, value) creates a GasCoin MoveObject
    // with type_ = GasCoin, has_public_transfer = true, and BCS-encoded Coin<SUI> contents
    let gas_coin_move_object = MoveObject::new_gas_coin(
        gas_object_version,        // version: 289568467
        gas_object_id,             // id: 0x400dbdcf...
        966004240u64,              // balance: 966004240 MIST (≈ 0.966 SUI)
    );

    // Wrap in a full Object with owner, previous_transaction, and storage_rebate
    let previous_tx = TransactionDigest::new(decode_b58_digest(
        "8sh9SEYmeAzFMbVJwB2CoQKaBLAZeWVEASVemgdshsKC",
    ));
    println!("Previous transaction digest: {:#02X?}", decode_b58_digest("8sh9SEYmeAzFMbVJwB2CoQKaBLAZeWVEASVemgdshsKC"));

    // Object::new_move sets storage_rebate to 0 by default.
    // We use DerefMut (via Arc::make_mut) to set the actual storage_rebate value.
    let mut obj = Object::new_move(
        gas_coin_move_object,
        Owner::AddressOwner(sender),
        previous_tx,
    );
    obj.storage_rebate = 988000;

    let object_bytes = bcs::to_bytes(&obj).unwrap();
    let object_base64 = base64::prelude::BASE64_STANDARD.encode(&object_bytes);
    println!("Object base64: {}", object_base64);

    let ref_obj = "AAEB03ZCEQAAAAAoQA29z+2o4eZOu/VxDM9aZ7+m5/lFySvIYR4MtEtyGd4QDpQ5AAAAAABvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeISB0/j3Uc6ljNbb1tbWgvj5PAz7MCgIO6e91iU9asLM9x2ATDwAAAAAA";
    assert_eq!(ref_obj, object_base64, "Object mismatch!");
    println!("Object matches reference ✓");

    bs58::decode("7vBQVjLUYjJ2kiA4YYhwrHey38LqDVU3FMC3PtUgpotV")
        .into_vec()
        .unwrap()
        .iter()
        .for_each(|b| print!("{:02X}", b));
    println!();

    // =========================================================================
    // 3) Print ObjectRef for convenience (used in gas_payment field)
    // =========================================================================
    let object_ref: ObjectRef = (gas_object_id, gas_object_version, gas_object_digest);
    let ref_base64 = base64::prelude::BASE64_STANDARD.encode(bcs::to_bytes(&object_ref).unwrap());
    println!("ObjectRef base64: {}", ref_base64);
}
