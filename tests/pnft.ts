import {
  Connection,
  Keypair,
  PublicKey,
  Transaction,
  SYSVAR_INSTRUCTIONS_PUBKEY,
  LAMPORTS_PER_SOL,
  sendAndConfirmTransaction,
  ComputeBudgetProgram,
  SystemProgram,
} from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  getAssociatedTokenAddressSync,
} from "@solana/spl-token";
import {
  AssetData,
  CreateInstructionAccounts,
  CreateInstructionArgs,
  Metadata,
  MintInstructionAccounts,
  MintInstructionArgs,
  PROGRAM_ID,
  Payload,
  TokenStandard,
  TransferInstructionAccounts,
  TransferInstructionArgs,
  createCreateInstruction,
  createMintInstruction,
  createTransferInstruction,
} from "@metaplex-foundation/mpl-token-metadata";
import { PROGRAM_ID as TOKEN_AUTH_RULES_ID } from "@metaplex-foundation/mpl-token-auth-rules";
import { bignum } from "@metaplex-foundation/beet";

type CreateDigitalAssetArgs = {
  name: string;
  symbol: string;
  uri: string;
};

export const DEFAULT_PASS_RULESET = new PublicKey(
  "eBJLFYPxJmMGKuFwpDWkzxZeUrad92kZRC5BJLpzyT9"
);

export async function create(
  connection: Connection,
  payer: PublicKey,
  ruleSet: PublicKey | null,
  digitalAssetArgs: CreateDigitalAssetArgs
) {
  const { name, symbol, uri } = digitalAssetArgs;

  const mintKp = Keypair.generate();
  const mint = mintKp.publicKey;

  const [metadata] = PublicKey.findProgramAddressSync(
    [Buffer.from("metadata"), PROGRAM_ID.toBuffer(), mint.toBuffer()],
    PROGRAM_ID
  );
  const [masterEdition] = PublicKey.findProgramAddressSync(
    [
      Buffer.from("metadata"),
      PROGRAM_ID.toBuffer(),
      mint.toBuffer(),
      Buffer.from("edition"),
    ],
    PROGRAM_ID
  );

  // Create the initial asset and ensure it was created successfully
  const assetData: AssetData = {
    name,
    symbol,
    uri,
    sellerFeeBasisPoints: 0,
    creators: [
      {
        address: payer,
        share: 100,
        verified: false,
      },
    ],
    primarySaleHappened: false,
    isMutable: true,
    tokenStandard: TokenStandard.ProgrammableNonFungible,
    // collection: collection ? { key: collection, verified: false } : null,
    collection: null,
    uses: null,
    collectionDetails: null,
    ruleSet,
  };

  const accounts: CreateInstructionAccounts = {
    metadata,
    masterEdition,
    mint: mint,
    authority: payer,
    payer: payer,
    splTokenProgram: TOKEN_PROGRAM_ID,
    sysvarInstructions: SYSVAR_INSTRUCTIONS_PUBKEY,
    updateAuthority: payer,
  };

  const args: CreateInstructionArgs = {
    createArgs: {
      __kind: "V1",
      assetData,
      decimals: 0,
      printSupply: { __kind: "Zero" },
    },
  };

  const tx = new Transaction();
  let createIx = createCreateInstruction(accounts, args);
  for (let i = 0; i < createIx.keys.length; i++) {
    if (createIx.keys[i].pubkey.toBase58() === mint.toBase58()) {
      createIx.keys[i].isSigner = true;
      createIx.keys[i].isWritable = true;
    }
  }
  tx.add(createIx);
  tx.feePayer = payer;
  tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;

  return { tx, mintKp, metadata };
}

export async function transfer(
  connection: Connection,
  amount: number,
  metadata: PublicKey,
  owner: PublicKey,
  to: PublicKey,
  authority: PublicKey
) {
  let meta = await Metadata.fromAccountAddress(
    connection,
    metadata,
    "confirmed"
  );
  let authorizationRules = meta.programmableConfig.ruleSet;

  let token = getAssociatedTokenAddressSync(meta.mint, owner);
  let destination = getAssociatedTokenAddressSync(meta.mint, to);

  const [masterEdition] = PublicKey.findProgramAddressSync(
    [
      Buffer.from("metadata"),
      PROGRAM_ID.toBuffer(),
      meta.mint.toBuffer(),
      Buffer.from("edition"),
    ],
    PROGRAM_ID
  );

  const transferAcccounts: TransferInstructionAccounts = {
    authority: authority,
    tokenOwner: new PublicKey(owner),
    token,
    metadata: new PublicKey(metadata),
    mint: meta.mint,
    edition: masterEdition,
    destinationOwner: to,
    destination,
    payer: authority,
    splTokenProgram: TOKEN_PROGRAM_ID,
    splAtaProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
    systemProgram: SystemProgram.programId,
    sysvarInstructions: SYSVAR_INSTRUCTIONS_PUBKEY,
    authorizationRules,
    authorizationRulesProgram: TOKEN_AUTH_RULES_ID,
    ownerTokenRecord: findTokenRecordPda(meta.mint, token),
    destinationTokenRecord: findTokenRecordPda(meta.mint, destination),
  };

  const args = {
    __kind: "V1",
    amount: amount as bignum,
    authorizationData: null,
  };
  const transferArgs: TransferInstructionArgs = {
    transferArgs: args as any,
  };
  const transferIx = createTransferInstruction(transferAcccounts, transferArgs);
  const modifyComputeUnits = ComputeBudgetProgram.setComputeUnitLimit({
    units: 400_000,
  });

  const tx = new Transaction();
  tx.add(modifyComputeUnits).add(transferIx);
  return tx;
}

export async function mintPnft(
  connection: Connection,
  metadata: PublicKey,
  owner: PublicKey,
  amount: number
) {
  const meta = await Metadata.fromAccountAddress(
    connection,
    metadata,
    "confirmed"
  );
  const authConfig = meta.programmableConfig;
  const [masterEdition] = PublicKey.findProgramAddressSync(
    [
      Buffer.from("metadata"),
      PROGRAM_ID.toBuffer(),
      meta.mint.toBuffer(),
      Buffer.from("edition"),
    ],
    PROGRAM_ID
  );

  let token = getAssociatedTokenAddressSync(meta.mint, owner);
  let tokenRecord = findTokenRecordPda(meta.mint, token);
  const mintAcccounts: MintInstructionAccounts = {
    token,
    tokenOwner: owner,
    metadata,
    masterEdition,
    tokenRecord,
    mint: meta.mint,
    payer: owner,
    authority: owner,
    sysvarInstructions: SYSVAR_INSTRUCTIONS_PUBKEY,
    splAtaProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
    splTokenProgram: TOKEN_PROGRAM_ID,
    authorizationRules: authConfig ? authConfig.ruleSet : null,
    authorizationRulesProgram: TOKEN_AUTH_RULES_ID,
  };

  const payload: Payload = {
    map: new Map(),
  };

  let authorizationData = {
    payload,
  };

  const mintArgs: MintInstructionArgs = {
    mintArgs: {
      __kind: "V1",
      amount,
      authorizationData,
    },
  };

  const mintIx = createMintInstruction(mintAcccounts, mintArgs);
  const tx = new Transaction();
  tx.add(mintIx);
  return tx;
}

export function findTokenRecordPda(
  mint: PublicKey,
  token: PublicKey
): PublicKey {
  return PublicKey.findProgramAddressSync(
    [
      Buffer.from("metadata"),
      PROGRAM_ID.toBuffer(),
      mint.toBuffer(),
      Buffer.from("token_record"),
      token.toBuffer(),
    ],
    PROGRAM_ID
  )[0];
}
