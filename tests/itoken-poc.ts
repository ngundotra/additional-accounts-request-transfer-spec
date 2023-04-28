import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { TokenProgram } from "../target/types/token_program";
import { TokenWrapper } from "../target/types/token_wrapper";
import {
  TOKEN_PROGRAM_ID,
  MINT_SIZE,
  getMinimumBalanceForRentExemptMint,
  createInitializeMint2Instruction,
  createMintToInstruction,
  createAssociatedTokenAccountInstruction,
  getAssociatedTokenAddressSync,
} from "@solana/spl-token";
import {
  Transaction,
  Message,
  VersionedTransaction,
  MessageV0,
  SystemProgram,
  Keypair,
  PublicKey,
  AccountMeta,
  RpcResponseAndContext,
  SimulatedTransactionResponse,
  TransactionInstruction,
} from "@solana/web3.js";
import { base64 } from "@coral-xyz/anchor/dist/cjs/utils/bytes";

import { DEFAULT_PASS_RULESET, create, mintPnft } from "./pnft";

async function resolveRemainingAccounts<I extends anchor.Idl>(
  program: anchor.Program<I>,
  instructions: TransactionInstruction[]
): Promise<AccountMeta[]> {
  // Simulate transaction
  let message = MessageV0.compile({
    payerKey: program.provider.publicKey!,
    instructions,
    recentBlockhash: (await program.provider.connection.getRecentBlockhash())
      .blockhash,
  });
  let transaction = new VersionedTransaction(message);
  let simulationResult = await program.provider.connection.simulateTransaction(
    transaction,
    { commitment: "confirmed" }
  );

  // When the simulation RPC response is fixed, then the following code will work
  // but until then, we have to parse the logs manually
  // ===============================================================
  // let returnDataTuple = simulationResult.value.returnData;
  // let [b64Data, encoding] = returnDataTuple["data"];
  // if (encoding !== "base64") {
  //   throw new Error("Unsupported encoding: " + encoding);
  // }
  // ===============================================================
  let logs = simulationResult.value.logs;
  let b64Data = logs[logs.length - 2].split(" ")[3];
  let data = base64.decode(b64Data);

  // We start deserializing the Vec<IAccountMeta> from the 5th byte
  // The first 4 bytes are u32 for the Vec of the return data
  let numBytes = data.slice(0, 4);
  let numMetas = new anchor.BN(numBytes, null, "le");
  let offset = 4;

  let realAccountMetas: AccountMeta[] = [];
  let coder = program.coder.types;
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

describe("itoken-poc", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const wallet = anchor.getProvider().publicKey!;

  describe("Token Wrapper", () => {
    // iProgram

    const wrapper = anchor.workspace.TokenWrapper as Program<TokenWrapper>;
    const tokenkeg = TOKEN_PROGRAM_ID;
    const iProgram = anchor.workspace.TokenProgram as Program<TokenProgram>;

    // Tokenkeg
    let destination: PublicKey = Keypair.generate().publicKey;
    let tokenMint: PublicKey;
    let ata: PublicKey;
    let destinationAta: PublicKey;

    // pnft
    let pnftMetadata: PublicKey;

    it("Can initialize a interface program", async () => {
      // Add your test here.
      let tx = await iProgram.methods
        .initializeMint(new anchor.BN(10))
        .accounts({
          authority: wallet,
        })
        .rpc({ skipPreflight: true });
      console.log("Initialized iProgram", tx);
    });
    it("Can initialize a mint", async () => {
      Keypair.generate();
      Keypair.generate();
      Keypair.generate();

      const mintKp = Keypair.generate();
      tokenMint = mintKp.publicKey;

      ata = getAssociatedTokenAddressSync(tokenMint, wallet);
      destinationAta = getAssociatedTokenAddressSync(tokenMint, destination);

      const lamports = await getMinimumBalanceForRentExemptMint(
        wrapper.provider.connection
      );
      const transaction = new Transaction().add(
        SystemProgram.createAccount({
          fromPubkey: wallet,
          newAccountPubkey: tokenMint,
          space: MINT_SIZE,
          lamports,
          programId: tokenkeg,
        }),
        createInitializeMint2Instruction(
          tokenMint,
          9,
          wallet,
          wallet,
          tokenkeg
        ),
        createAssociatedTokenAccountInstruction(
          wallet,
          ata,
          wallet,
          tokenMint,
          tokenkeg
        ),
        createMintToInstruction(tokenMint, ata, wallet, 10)
      );

      let tx = await wrapper.provider.sendAndConfirm(transaction, [mintKp], {
        skipPreflight: true,
      });
      console.log("Initialized token mint & ata:", tx);
    });
    it("Can transfer iProgram using wrapper", async () => {
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

      let keys = await resolveRemainingAccounts(wrapper, [
        preflightInstruction,
      ]);

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
      console.log("Transferred iProgram with wrapper", tx);
    });
    it("Can initialize a pnft", async () => {
      let {
        metadata: metadataKey,
        mintKp,
        tx,
      } = await create(
        wrapper.provider.connection,
        wallet,
        DEFAULT_PASS_RULESET,
        {
          name: "test",
          symbol: "test",
          uri: "test",
        }
      );

      pnftMetadata = metadataKey;

      let txId = await wrapper.provider.sendAndConfirm(tx, [mintKp], {
        skipPreflight: true,
        commitment: "confirmed",
      });
      console.log("created pnft with txId: ", txId, pnftMetadata.toBase58());

      tx = await mintPnft(wrapper.provider.connection, pnftMetadata, wallet, 1);
      txId = await wrapper.provider.sendAndConfirm(tx, [], {
        skipPreflight: true,
        commitment: "confirmed",
      });
      console.log("minted pnft with txId: ", txId);
    });
    it("Can transfer pnft using wrapper", async () => {
      const ix = await wrapper.methods
        .preflightTransfer(new anchor.BN(1))
        .accounts({
          to: destination,
          owner: wallet,
          mint: pnftMetadata,
          authority: wallet,
        })
        .instruction();
      let keys = await resolveRemainingAccounts(wrapper, [ix]);

      const txId = await wrapper.methods
        .transfer(new anchor.BN(1))
        .accounts({
          to: destination,
          owner: wallet,
          mint: pnftMetadata,
          authority: wallet,
        })
        .remainingAccounts(keys)
        .rpc({ skipPreflight: true, commitment: "confirmed" });
      console.log("transferred pnft with txId: ", txId);
    });
    it("Can transfer tokenkeg using wrapper", async () => {
      let preInstructions = [
        createAssociatedTokenAccountInstruction(
          wallet,
          destinationAta,
          destination,
          tokenMint,
          tokenkeg
        ),
      ];

      let preflightInstruction = await wrapper.methods
        .preflightTransfer(new anchor.BN(1))
        .accounts({
          to: destination,
          owner: wallet,
          mint: tokenMint,
          authority: wallet,
        })
        .remainingAccounts([])
        .preInstructions([])
        .instruction();

      let keys = await resolveRemainingAccounts(wrapper, [
        ...preInstructions,
        preflightInstruction,
      ]);

      let instruction = await wrapper.methods
        .transfer(new anchor.BN(1))
        .accounts({
          to: destination,
          owner: wallet,
          mint: tokenMint,
          authority: wallet,
        })
        .remainingAccounts(keys)
        .preInstructions([])
        .instruction();
      let message = MessageV0.compile({
        payerKey: wallet,
        instructions: [...preInstructions, instruction],
        recentBlockhash: (
          await wrapper.provider.connection.getRecentBlockhash()
        ).blockhash,
      });
      let transaction = new VersionedTransaction(message);
      let tx = await wrapper.provider.sendAndConfirm(transaction, [], {
        skipPreflight: true,
      });

      console.log("Transferred spl token with wrapper", tx);
    });
  });
});
