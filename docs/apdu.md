# APDUs

The messaging format of the app uses the [Block Protocol](/docs/block-protocol.md), which is an application level protocol built on top of the [APDU protocol](https://developers.ledger.com/docs/nano-app/application-structure/#apdu-interpretation-loop).

All commands use `CLA = 0x00`.
The `P1` and `P2` fields are reserved for future use and must be set to `0` in all messages.

| CLA | INS | COMMAND NAME                     | DESCRIPTION                                                       |
|-----|-----|----------------------------------|-------------------------------------------------------------------|
| 00  | 00  | GET_VERSION                      | Gets the app version in machine readable format (bytes)           |
| 00  | 01  | VERIFY_ADDRESS                   | Shows the Address on device for a BIP32 path                      |
| 00  | 02  | GET_PUBKEY                       | Gets the Public Key and Address for a BIP32 path                  |
| 00  | 03  | SIGN_TX                          | Sign Transaction                                                  |
| 00  | 22  | PROVIDE_TOKEN_DYNAMIC_DESCRIPTOR | Provides a signed token descriptor (ticker, magnitude, coin type) |
| 00  | FE  | GET_VERSION_STR                  | Gets the app version in string                                    |
| 00  | FF  | QUIT_APP                         | Quits the app                                                     |

### GET_VERSION

Returns the version of the app currently running on the Ledger in machine readable format (bytes)

#### Encoding

**Command**

| *CLA* | *INS* |
|-------|-------|
| 00    | 00    |

**Output data**

| Length       | Description     |
|--------------|-----------------|
| `1`          | Major version   |
| `1`          | Minor version   |
| `1`          | Patch version   |
| `<variable>` | Name of the app |

### GET_VERSION_STR

Returns the version of the app currently running on the Ledger as an ASCII string, for example `1.5.14`.

#### Encoding

**Command**

| *CLA* | *INS* |
|-------|-------|
| 00    | FE    |

**Output data**

| Length       | Description                            |
|--------------|----------------------------------------|
| `<variable>` | Version string, `major.minor.patch`    |

### VERIFY_ADDRESS

Shows the address for the given derivation path, and returns the public key and the address.

#### Encoding

**Command**

| *CLA* | *INS* |
|-------|-------|
| 00    | 01    |

**Input data**

| Length | Name              | Description                            |
|--------|-------------------|----------------------------------------|
| `1`    | `n`               | Number of derivation steps             |
| `4`    | `bip32_path[0]`   | First derivation step (little endian)  |
| `4`    | `bip32_path[1]`   | Second derivation step (little endian) |
|        | ...               |                                        |
| `4`    | `bip32_path[n-1]` | `n`-th derivation step (little endian) |

**Output data**

| Length       | Description                  |
|--------------|------------------------------|
| `1`          | The length of the public key |
| `<variable>` | Public key                   |
| `1`          | The length of the address    |
| `<variable>` | Address                      |

### GET_PUBKEY

Returns the public key and the address for the given derivation path.

#### Encoding

**Command**

| *CLA* | *INS* |
|-------|-------|
| 00    | 02    |

**Input data**

##### Parameter 1

| Length | Name              | Description                            |
|--------|-------------------|----------------------------------------|
| `1`    | `n`               | Number of derivation steps             |
| `4`    | `bip32_path[0]`   | First derivation step (little endian)  |
| `4`    | `bip32_path[1]`   | Second derivation step (little endian) |
|        | ...               |                                        |
| `4`    | `bip32_path[n-1]` | `n`-th derivation step (little endian) |

**Output data**

| Length       | Description                  |
|--------------|------------------------------|
| `1`          | The length of the public key |
| `<variable>` | Public key                   |
| `1`          | The length of the address    |
| `<variable>` | Address                      |

### SIGN_TX

Sign a Transaction, using the key for the given derivation path

#### Encoding

**Command**

| *CLA* | *INS* |
|-------|-------|
| 00    | 03    |

**Input data**

##### Parameter 1

| Length    | Name      | Description                         |
|-----------|-----------|-------------------------------------|
| `4`       | `tx_size` | Size of transaction (little endian) |
| `tx_size` | `tx`      | Transaction                         |

##### Parameter 2

| Length | Name              | Description                            |
|--------|-------------------|----------------------------------------|
| `1`    | `n`               | Number of derivation steps             |
| `4`    | `bip32_path[0]`   | First derivation step (little endian)  |
| `4`    | `bip32_path[1]`   | Second derivation step (little endian) |
|        | ...               |                                        |
| `4`    | `bip32_path[n-1]` | `n`-th derivation step (little endian) |

##### Parameter 3 (required only for clear signing of certain transactions)

For clear signing of certain transactions in which the coin type and amount being transferred cannot be obtained from the transaction itself, the object data of the objects referenced in the transaction are required. In those cases the object data should be provided as the third parameter by appending the length prefixed data of each of object as described below. The order of objects in this list is irrelevant. Any object which cannot be parsed, or which is not required for obtaining the information will be ignored.

It is advisable to provide the object data of all the coin type objects referenced in "gas_payment" and "inputs" of the transaction.
In the abscence of this info the user may get a blind signing prompt.

**SIP-58**: The app supports transactions with empty `gas_data.payment` (gas paid from address balance). In that case, parameter 3 may be omitted or contain only objects from "inputs". Stateless transactions (empty gas payment) **require** `TransactionExpiration::ValidDuring` for replay protection; transactions with empty payment and `None` or `Epoch` expiration will be rejected.

| Length             | Name               | Description                        |
|--------------------|--------------------|------------------------------------|
| `4`                | `n`                | Number of objects (little endian)  |
| `4`                | `object[0].length` | Length of object 0 (little endian) |
| `object[0].length` | `object[0]`        | Object data                        |
|                    | ...                |                                    |
| `4`                | `object[n].length` | Length of object n (little endian) |
| `object[n].length` | `object[n]`        | Object data                        |

**Output data**

| Length       | Description     |
|--------------|-----------------|
| `<variable>` | Signature bytes |


### PROVIDE_TOKEN_DYNAMIC_DESCRIPTOR

Provides a token dynamic descriptor by parsing TLV (Type-Length-Value) data.
This APDU enables the app receive tokens information dynamically; new tokens support will not require new app code changes, and the data is signed to confirm its trusted source. Without this APDU, token transactions will not show to the user the correct ticker and value.

*The APDU needs to be sent before the SIGN_TX one.*

*The app supports one token dynamic descriptor per time, if another token transaction needs to be signed, the token dynamic descriptor APDU needs to be sent with the new token data before the signing.*

#### Encoding

**Command**

| *CLA* | *INS* |
|-------|-------|
| 00    | 22    |

**Input data**

The input is a length-prefixed TLV blob:

| Length     | Name       | Description                                        |
|------------|------------|----------------------------------------------------|
| `2`        | `tlv_size` | Size of the TLV data, at most 1024 (little endian) |
| `tlv_size` | `tlv`      | TLV data with the following structure              |

| Field          | Tag  | Length     | Type               | Description                                      |
|----------------|------|------------|--------------------|--------------------------------------------------|
| STRUCTURE_TYPE | 0x01 | 1 byte     | required byte      | Overall structure type                           |
| VERSION        | 0x02 | 1 byte     | required byte      | Version of the serialization format              |
| COIN_TYPE      | 0x03 | 4 bytes    | required bytes[]   | Coin Type as defined in SLIP-44                  |
| APP_NAME       | 0x04 | var        | required string    | Name of the App/Coin. Case sensitive.            |
| TICKER         | 0x05 | var        | required string    | Token ticker displayed on the device             |
| MAGNITUDE      | 0x06 | 1 byte     | required uint      | Token magnitude                                  |
| TUID           | 0x07 | var        | required TLV       | Token unique identifier (nested TLV structure)   |
| SIGNATURE      | 0x08 | var        | required bytes[]   | Signature for validation                         |

##### TUID field
| Field                 | Tag  | Length   | Type             | Description                      |
|:----------------------|:-----|:---------|:-----------------|:---------------------------------|
| TOKEN_PACKAGE_ADDRESS | 0x10 | 32 bytes | required bytes[] | Package address (fixed 32 bytes) |
| TOKEN_MODULE_NAME     | 0x11 | var      | required string  | Module name                      |
| TOKEN_STRUCT_NAME     | 0x12 | var      | required string  | Function name                    |

**Output data**

| Length | Description |
|--------|-------------|
| -      | No output data; success indicated by SW_OK, or error status |


## Status Words

When a command handler rejects, the status word sent to the host is the code it was rejected with, as is. Failures of the block protocol itself are sent as `0x6800` plus a `SyscallError` value. The status words below are the ones this app can produce.

### Command results

| SW     | Name                  | Description                                                                                                                                     |
|--------|-----------------------|-------------------------------------------------------------------------------------------------------------------------------------------------|
| 0x9000 | `Ok`                  | Success, or continue if more input from the client is expected                                                                                  |
| 0x6985 | `UserCancelled`       | The user refused on the device                                                                                                                  |
| 0x6E01 | `BadIns`              | No command exists for the `CLA`, `INS`, `P1` and `P2`, or `VERIFY_ADDRESS` was sent during a swap                                               |
| 0x6E05 | `SwapTxParamMismatch` | Swap transaction parameters check failed                                                                                                        |
| 0x0002 | `InvalidParameter`    | Bad input, for example a BIP32 path that does not start with `m/44'/784'`                                                                       |
| 0x0003 | `Overflow`            | The reply did not fit its output buffer (`VERIFY_ADDRESS`, `GET_PUBKEY`)                                                                        |
| 0x0008 | `NotSupported`        | The transaction is not supported: it is not recognized and blind signing is disabled or a swap is active, or the parser refused it as ambiguous |
| 0x0009 | `InvalidState`        | Internal state is inconsistent, for example a parsed transaction is missing its principals                                                      |
| 0x000C | `Unspecified`         | A cryptographic operation failed for a reason that has no other code                                                                            |
| 0x6850 | `ParseError`          | The transaction or object data could not be parsed                                                                                              |

### `PROVIDE_TOKEN_DYNAMIC_DESCRIPTOR` errors

The status word is `0x7000` plus the TLV error number.

| SW     | TLV error                     | Description                                                                  |
|--------|-------------------------------|------------------------------------------------------------------------------|
| 0x7000 | `UnexpectedEof`               | The descriptor is truncated, longer than 1024 bytes, or a field does not fit |
| 0x7001 | `InvalidDerLength`            | Invalid DER length                                                           |
| 0x7002 | `LengthOverflow`              | Length overflow                                                              |
| 0x7003 | `UnknownTag`                  | Unknown tag                                                                  |
| 0x7004 | `HandlerFailed`               | A field handler failed                                                       |
| 0x7005 | `DuplicateUniqueTag`          | A tag that must be unique appears more than once                             |
| 0x7006 | `SignatureVerificationFailed` | The descriptor signature did not verify                                      |
| 0x7007 | `MissingMandatoryTag`         | A mandatory tag is missing                                                   |

### Block protocol errors

| SW     | Description                                                                                            |
|--------|--------------------------------------------------------------------------------------------------------|
| 0x6802 | The block protocol input is malformed (`InvalidParameter`)                                             |
| 0x6809 | The block protocol is in an invalid state, or a handler ended without sending a reply (`InvalidState`) |

### Device and SDK errors

| SW     | Name             | Description                                                                              |
|--------|------------------|------------------------------------------------------------------------------------------|
| 0x6E03 | `BadLen`         | The APDU length is invalid                                                               |
| 0x6901 | `CmdNotAccepted` | An APDU arrived while another command was still in progress, for example during a review |
| 0x5515 | `DeviceLocked`   | The device is locked                                                                     |
| 0xE000 | `Panic`          | The app panicked                                                                         |
