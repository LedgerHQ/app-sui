# Block Protocol

This is an application level protocol built on top of the [APDU protocol](https://developers.ledger.com/docs/nano-app/application-structure/#apdu-interpretation-loop).

Using this protocol the Ledger app can recieve data of arbitrary sizes from the host.
It also allows the Ledger app to store arbitrary data on the host, which can be later recieved using the hash of the data.

The hashing is done using SHA256, and the Hash size is 32 bytes.

In this protocol the Host executes the same APDU multiple times, ie the `CLA`, `INS`, `P1` and `P2` fields should be same for all APDU messages.

All the messages sent by the host and the responses from the Ledger app contain a block protocol message header in the first byte followed by a data payload of arbitrary length.

### APDU Command 

| Length       | Name           | Description                               |
|--------------|----------------|-------------------------------------------|
| `1`          | `HostToLedger` | Block protocol's HostToLedger instruction |
| `<variable>` | `payload`      | Data payload of arbitrary length          |


| HostToLedger instruction     | Value | Payload                     |
|------------------------------|-------|-----------------------------|
| START                        | 0     | Hashes of parameters        |
| GET_CHUNK_RESPONSE_SUCCESS   | 1     | Data for the requested hash |
| GET_CHUNK_RESPONSE_FAILURE   | 2     | empty                       |
| PUT_CHUNK_RESPONSE           | 3     | empty                       |
| RESULT_ACCUMULATING_RESPONSE | 4     | empty                       |

### Response from Ledger

The Ledger App responds to each of the APDU commands with the status code `StatusWords::OK` (`0x9000`), and the APDU response contains the block protocol's next instruction for the host side.

If the Ledger App responds with any other status word, then the host should terminate the block protocol and throw an error.

| Length       | Name           | Description                               |
|--------------|----------------|-------------------------------------------|
| `1`          | `LedgerToHost` | Block protocol's LedgerToHost instruction |
| `<variable>` | `payload`      | Data payload of arbitrary length          |


| LedgerToHost instruction | Value | Payload                                              |
|--------------------------|-------|------------------------------------------------------|
| RESULT_ACCUMULATING      | 0     | Data to be appended to existing return value, if any |
| RESULT_FINAL             | 1     | Data to be appended to existing return value, if any |
| GET_CHUNK                | 2     | Hash of data chunk                                   |
| PUT_CHUNK                | 3     | Data to be stored on the host                        |

## Sequence Diagram

The following diagram illustrates a typical transaction signing flow using the Block Protocol:

```mermaid
sequenceDiagram
    participant Host
    participant Ledger
    
    Note over Host: Chunk transaction into blocks<br/>(180 bytes each)
    Note over Host: Compute SHA256 hashes<br/>for each block
    
    Host->>Ledger: APDU: START [hash1]
    Note over Ledger: Store hash1,<br/>prepare to request data
    
    Ledger->>Host: GET_CHUNK [hash1]
    Note over Host: Retrieve block1<br/>by hash
    
    Host->>Ledger: GET_CHUNK_RESPONSE_SUCCESS<br/>[hash2][data_0-179]
    Note over Ledger: Verify: SHA256(response) == hash1<br/>Extract next hash: hash2
    
    Ledger->>Host: GET_CHUNK [hash2]
    Host->>Ledger: GET_CHUNK_RESPONSE_SUCCESS<br/>[hash3][data_180-359]
    Note over Ledger: Verify: SHA256(response) == hash2<br/>Extract next hash: hash3
    
    Ledger->>Host: GET_CHUNK [hash3]
    Host->>Ledger: GET_CHUNK_RESPONSE_SUCCESS<br/>[0x00...][data_360-499]
    Note over Ledger: Verify: SHA256(response) == hash3<br/>Zero hash = last block<br/>Transaction complete!
    
    Note over Ledger: Parse transaction<br/>Display to user<br/>User approves
    
    Ledger->>Host: RESULT_FINAL [signature]
    Note over Host: Transaction signed!<br/>Status: 0x9000 (OK)
```

## Protocol steps

* The host does chunking of input parameters, as described in [Chunking of Input Parameters](#chunking-of-input-parameters).

* The host initiates the protocol by calling the APDU with the `START` instruction.
  The payload of this message contains the hashes of each of the parameter's first blocks, concatenated together.

* The Ledger app stores all the hashes sent with the `START` in its memory, and can request the input parameters by doing `GET_CHUNK` request(s).
  The payload of the `GET_CHUNK` request should be the hash of the block being requested.

* The Host must respond to the `GET_CHUNK` request in the next APDU call with either `GET_CHUNK_RESPONSE_SUCCESS` or `GET_CHUNK_RESPONSE_FAILURE`.
  When the host has the block for the requested hash, it must be sent as a payload of `GET_CHUNK_RESPONSE_SUCCESS`.
  
* When the Ledger app recieves the requested block as part of the `GET_CHUNK_RESPONSE_SUCCESS` command, it does the hashing of the payload data on the device to crosscheck the validity of data.
  If the hash of recieved data does not match the hash sent in the previous `GET_CHUNK` command, the Ledger app will terminate the execution of the APDU with an error.

* The Ledger app can optionally store any arbitrary data on the Host using the `PUT_CHUNK` command.
  The data to be stored is the payload of `PUT_CHUNK` command.
  The host must respond to this command with the `PUT_CHUNK_RESPONSE` in the next APDU call.

  The host must store this data in its memory for the entirity of this APDU's execution, as the Ledger app can possibly retrieve this data using its hash. 

* The Ledger app can optionally use `RESULT_ACCUMULATING` command to incrementally send the result data to the Host.
  The Host must store this data in its memory by appending it to any existing `result` values, and then respond to the Ledger app with `RESULT_ACCUMULATING_RESPONSE`.

* The `RESULT_FINAL` command is the last command sent by the Ledger app, the host must append the payload of this to the `result`, and return the `result` value.

## Chunking of Input Parameters

For many Ledger operations, like signing, the app requires multiple input parameters, each of which could be big in size.

A single APDU command has a limit on the size of data that can be sent, so in order to support input parameters of arbitrary sizes, each parameter is broken down into smaller chunks (of size 180 bytes).

All of these chunks are then chained together into data blocks, such that the first 32 bytes of each data block consists of the hash of the next block, and the rest of the bytes are the data of the input parameter.
The last block of this chain contains all zeroes in its first 32 bytes, indicating that this block is the end for this input parameter.

A single block of data is sent to the Ledger app in one APDU call, as the payload of `GET_CHUNK_RESPONSE_SUCCESS`.
Since the hash of the next block is part of the current block, the Ledger app can request the next block of data using the `GET_CHUNK` request.
