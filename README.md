# SRFC 00010 - Additional Accounts Request - Transfer Spec

This spec is currently alpha and subject to change

## Summary
A standard protocol to enable on-chain and client communication with Solana programs to "transfer" assets that allows target programs to require additional accounts.

## Motivation
A standard protocol for enabling programs to support "transfer"ring assets while also allowing a flexible number of accounts into the program allows for a better user experience across apps and wallets in the Solana ecosystem.

By defining a protocol to resolve additional accounts required for programs to adhere to the same instruction interface, developers can build applications that are compatible with a wide range of programs.

Calling programs should ensure that called programs are using the additional accounts appropriately, or otherwise fail instruction execution. 

Developers implementing this specification should be prepared to chew glass.

By standardizing a simple approach to solving program abstraction, we ensure basic compatibility of programs and clients so developers can focus on higher level abstractions.

## Specification: Program Trait - Transfer
<!-- A Solana Pay transfer request URL describes a non-interactive request for a SOL or SPL Token transfer. -->

Executing a "transfer" instruction against a program that implements this spec requires two CPIs from the caller program to the callee program. 
The first CPI from the caller to the callee is to determine which (if any) additional accounts are require for the 2nd CPI.
The second CPI from the caller to the callee is with the same list of accounts from the 1st call, but also passes the list of accounts requested by the first CPI.

The Additional Accounts Request spec for Transfers requires that programs implement two instructions, described below.

```rust
use anchor_lang::prelude::*;

/// Required Accounts
#[derive(Accounts)]
pub struct ITransfer<'info> {
    /// CHECK:
    pub owner: AccountInfo<'info>,
    /// CHECK:
    pub to: AccountInfo<'info>,
    pub authority: Signer<'info>,
    /// CHECK:
    pub mint: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct MyProgramTransfer {
    /// CHECK:
    pub owner: AccountInfo<'info>,
    /// CHECK:
    pub to: AccountInfo<'info>,
    pub authority: Signer<'info>,
    /// CHECK:
    pub mint: AccountInfo<'info>,
    
    // Additional optional accounts follow here
    pub my_special_account: AccountInfo<'info>,
    // etc
}


#[program]
pub mod MyProgram {
    pub fn preflight_transfer(ctx: Context<ITransfer>, amount: u64) -> Result<()> {
        // Your code goes here 
        set_return_data(
            &PreflightPayload {
                accounts: vec![
                    IAccountMeta {
                        pubkey: *my_special_account_key,
                        // You cannot request additional signer accounts
                        signer: false, 
                        // You may however request additional writable or readonly accounts
                        writable: true,
                    },
                ]
            }.try_to_vec()?
        )?;
        Ok(())
    }

    pub fn transfer(ctx: Context<MyTransfer>, amount: u64) -> Result<()> {
        // Your code goes here
        Ok(())
    }
}
enum MyProgramInstruction {
    ...,
    PreflightTransfer(u64)=solana_program::hash::hash("global:preflight_transfer")[..8],
    Transfer(u64)=solana_program::hash::hash("global:transfer")[..8]
}
```

Executing "transfer" against a conforming program is interactive because optional accounts may be sent in the 2nd CPI. The optional accounts are derived from the 1st CPI by Borsh deserializing return data as `Vec<AccountMeta>`.

# Accounts 

The accounts list required for adhering to this Transfer spec is simply a list of account metas, that have no direct relationship to each other.

We overlay semantic descriptions to give advice on how this should be used, but ultimately we expect that there will be program implementations that abuse the 
semantic descriptions.



## Owner
- isSigner: false
- isWritable: false

This is the owner of the asset to be transferred.

## To
- isSigner: false
- isWritable: false

This is the intended recipient of the transferred asset.

## Authority
- isSigner: true
- isWritable: false

This is the account that has the authority to transfer from owner to the recipient. For example, this may be the same pubkey as `owner`.

## Mint
- isSigner: false
- isWritable: false

This account was included for Token* compatability.
This account is meant to be your implementing program's program id, so calling programs know which program to execute.
Or, it can be used as a token* `Mint` account, which allows programs to decide if they need to execute a token* CPI or an interface-compliant "transfer".

# Instructions

The instructions formats are described below

### Amount
Both instructions have a single parameter `amount` which must be serialized & deserialized as a little-endian `u64`.

### `preflight_transfer`

This instruction's data has an 8 byte discriminantor: `[0x9d, 0x84, 0xf5, 0x5a, 0x61, 0xea, 0x7b, 0xe2]`, followed by u64 serialized in little-endian format.
And no other bytes.

The accounts to this instruction are:
```rust
vec![
    // owner
    AccountMeta {
        pubkey: owner,
        isSigner: false,
        isWritable: false,
    }
    // to
    AccountMeta {
        pubkey: to,
        isSigner: 
        isWritable:
    }
    // authority
    AccountMeta {
        pubkey: authority,
        isSigner: true,
        isWritable: false,
    }
    // mint
    AccountMeta {
        pubkey: mint
        isSigner: false,
        isWritable: false
    }

]
```

Return data for this instruction is a vector of `AccountMeta`s, serialized as `ReturnData`.

```rust
#[derive(BorshSerialize, BorshDeserialize)]
pub struct IAccountMeta {
    pub pubkey: Pubkey,
    pub signer: bool,
    pub writable: bool,
}

pub type ReturnData = Vec<IAccountMeta>;
```

### `transfer`

This instruction's data has an 8 byte discriminantor: `[0xa3, 0x34, 0xc8, 0xe7, 0x8c, 0x03, 0x45, 0xba]`, followed by u64 serialized in little-endian format.
And no other bytes.

The accounts to this instruction are:
```rust
vec![
    // owner
    AccountMeta {
        pubkey: owner,
        isSigner: false,
        isWritable: false,
    }
    // to
    AccountMeta {
        pubkey: to,
        isSigner: 
        isWritable:
    }
    // authority
    AccountMeta {
        pubkey: authority,
        isSigner: true,
        isWritable: false,
    }
    // mint
    AccountMeta {
        pubkey: mint
        isSigner: false,
        isWritable: false
    },
]
```
Additional account metas returned from the previous call to `preflight_transfer` must be appended to the list of accounts, in the order they were deserialized.


# Off-Chain Usage

In order to craft a `transfer` `TransactionInstruction` to a program that adheres to this spec, you can simulate the
`preflight_transfer` instruction with the required accounts, in order to get the list of additional `AccountMeta`s.

Then you can append those `AccountMeta`s to the remaining accounts.

Reference code is provided below, written using `@coral-xyz/anchor`.

```typescript
import * as anchor from '@coral-xyz/anchor';

async function resolveRemainingAccounts<I extends anchor.Idl>(
  program: anchor.Program<I>,
  simulationResult: RpcResponseAndContext<SimulatedTransactionResponse>
): Promise<AccountMeta[]> {
  let coder = program.coder.types;

  let returnDataTuple = simulationResult.value.returnData;
  let [b64Data, encoding] = returnDataTuple["data"];
  if (encoding !== "base64") {
    throw new Error("Unsupported encoding: " + encoding);
  }
  let data = base64.decode(b64Data);

  // We start deserializing the Vec<IAccountMeta> from the 5th byte
  // The first 4 bytes are u32 for the Vec of the return data
  let numBytes = data.slice(0, 4);
  let numMetas = new anchor.BN(numBytes, null, "le");
  let offset = 4;

  let realAccountMetas: AccountMeta[] = [];
  const metaSize = 34;
  for (let i = 0; i < numMetas.toNumber(); i += 1) {
    const start = offset + i * metaSize;
    const end = start + metaSize;
    let meta = coder.decode("ExternalIAccountMeta", data.slice(start, end));
    realAccountMetas.push({
      pubkey: meta.pubkey,
      isWritable: meta.writable,
      isSigner: meta.signer,
    });
  }
  return realAccountMetas;
}
```

This is used like so:
```typescript
// Simulate the `preflight_transfer` instruction
const preflightInstruction = await wrapper.methods
    .preflightTransfer(new anchor.BN(1))
    .accounts({
        to: destination,
        owner: wallet,
        authority: wallet,
        mint: iProgram.programId,
    })
    .remainingAccounts([])
    .instruction();

let message = MessageV0.compile({
    payerKey: wallet,
    instructions: [preflightInstruction],
    recentBlockhash: (
        await wrapper.provider.connection.getRecentBlockhash()
    ).blockhash,
});
let transaction = new VersionedTransaction(message);

// Deserialize the `AccountMeta`s from the return data
// We have to use VersionedTransactions to get `returnData`
// back from simulated transactions 
let keys = await resolveRemainingAccounts(
    wrapper,
    await wrapper.provider.connection.simulateTransaction(transaction)
);

// Send the actual `transfer` instruction with the required additional
// accounts
const tx = await wrapper.methods
    .transfer(new anchor.BN(1))
    .accounts({
        owner: wallet,
        to: destination,
        authority: wallet,
        mint: iProgram.programId,
    })
    .remainingAccounts(keys)
    .rpc({ skipPreflight: true });
console.log("Transferred with tx:", tx);
```

# Compatability: SPL Token 

SPL tokens are compatible with this format. 
There is a provided program `programs/token-wrapper` that shows how to "wrap" `tokenkeg` to make it compatible with this spec.

# Compatability: Token Metadata 

Programmable NFTs are compatible with this format. 
There is a provided program `programs/token-wrapper` that shows how to "wrap" `token-metadata` to make it compatible with this spec.

# Limitations

When returning a vector of account metas in the `preflight_transfer` instruction, additional account metas must have `isSigner: false`. 

Requiring additional `signer` account metas must come in the form of a new version of this specification.


# Reference

There is a reference implementation of a program adhering to this spec under `programs/token-program` of a program that records which `pubkey` owns how much of a token in a singleton address. 
The implementation is meant to mimic how ERC-20 tokens work.

Calling `transfer` on this program will change decrement the owner's stored balance by `amount` and increment the recipient's balance by `amount`. 


# Tests

To run a test against this program, run `anchor test`.
