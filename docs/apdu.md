# APDUs

The messaging format of the app uses the [Block Protocol](/docs/block-protocol.md), which is an application level protocol built on top of the [APDU protocol](https://developers.ledger.com/docs/nano-app/application-structure/#apdu-interpretation-loop).

All commands use `CLA = 0x00`.
The `P1` and `P2` fields are reserved for future use and must be set to `0` in all messages.

| CLA | INS | COMMAND NAME    | DESCRIPTION                                             |
|-----|-----|-----------------|---------------------------------------------------------|
| 00  | 00  | GET_VERSION     | Gets the app version in machine readable format (bytes) |
| 00  | 01  | VERIFY_ADDRESS  | Shows the Address on device for a BIP32 path            |
| 00  | 02  | GET_PUBKEY      | Gets the Public Key and Address for a BIP32 path        |
| 00  | 03  | SIGN_TX         | Sign Transaction                                        |
| 00  | FE  | GET_VERSION_STR | Gets the app version in string                          |
| 00  | FF  | QUIT_APP        | Quits the app                                           |

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

### VERIFY_ADDRESS

Shows the address for the given derivation path, and returns the public key and the address.

#### Encoding

**Command**

| *CLA* | *INS* |
|-------|-------|
| 00    | 01    |

**Input data**

| Length | Name              | Description                         |
|--------|-------------------|-------------------------------------|
| `1`    | `n`               | Number of derivation steps          |
| `4`    | `bip32_path[0]`   | First derivation step (big endian)  |
| `4`    | `bip32_path[1]`   | Second derivation step (big endian) |
|        | ...               |                                     |
| `4`    | `bip32_path[n-1]` | `n`-th derivation step (big endian) |

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

| Length | Name              | Description                         |
|--------|-------------------|-------------------------------------|
| `1`    | `n`               | Number of derivation steps          |
| `4`    | `bip32_path[0]`   | First derivation step (big endian)  |
| `4`    | `bip32_path[1]`   | Second derivation step (big endian) |
|        | ...               |                                     |
| `4`    | `bip32_path[n-1]` | `n`-th derivation step (big endian) |

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

| Length    | Name      | Description         |
|-----------|-----------|---------------------|
| `4`       | `tx_size` | Size of transaction |
| `tx_size` | `tx`      | Transaction         |

##### Parameter 2

| Length    | Name              | Description                         |
|-----------|-------------------|-------------------------------------|
| `1`       | `n`               | Number of derivation steps          |
| `4`       | `bip32_path[0]`   | First derivation step (big endian)  |
| `4`       | `bip32_path[1]`   | Second derivation step (big endian) |
|           | ...               |                                     |
| `4`       | `bip32_path[n-1]` | `n`-th derivation step (big endian) |

##### Parameter 3 (required only for clear signing of certain transactions)

For clear signing of certain transactions in which the coin type and amount being transferred cannot be obtained from the transaction itself, the object data of the objects referenced in the transaction are required. In those cases the object data should be provided as the third parameter by appending the length prefixed data of each of object as described below. The order of objects in this list is irrelevant. Any object which cannot be parsed, or which is not required for obtaining the information will be ignored.

It is advisable to provide the object data of all the coin type objects referenced in "gas_payment" and "inputs" of the transaction.
In the abscence of this info the user may get a blind signing prompt.

| Length             | Name               | Description                     |
|--------------------|--------------------|---------------------------------|
| `4`                | `n`                | Number of objects (big endian)  |
| `4`                | `object[0].length` | Length of object 0 (big endian) |
| `object[0].length` | `object[0]`        | Object data                     |
|                    | ...                |                                 |
| `4`                | `object[n].length` | Length of object n (big endian) |
| `object[n].length` | `object[n]`        | Object data                     |

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

The input is raw TLV data with the following structure:

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

| SW     | SW name                       | Description                                                |
|--------|-------------------------------|------------------------------------------------------------|
| 0x6808 | `SW_NOT_SUPPORTED`            | `INS` is disabled  (Blind Signing)                         |
| 0x6982 | `SW_NOTHING_RECEIVED`         | No input was received by the app                           |
| 0x6D00 | `SW_ERROR`                    | Error has occured due to bad input or user rejectected     |
| 0x6E00 | `SW_CLA_OR_INS_NOT_SUPPORTED` | No command exists for the `CLA` and `INS`                  |
| 0x6E01 | `SW_BAD_LEN`                  | Length mismatch in inputs                                  |
| 0x6E05 | `SW_SWAP_TX_PARAM_MISMATCH`   | Swap transaction parameters check failed                   |
| 0x9000 | `SW_OK`                       | Success, or continue if more input from client is expected |
