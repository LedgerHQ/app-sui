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

**Output data**

| Length       | Description     |
|--------------|-----------------|
| `<variable>` | Signature bytes |

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
