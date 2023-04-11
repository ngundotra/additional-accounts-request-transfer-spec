import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { ItokenPoc } from "../target/types/itoken_poc";
import { TokenProgram } from "../target/types/token_program";
import { TokenWrapper } from "../target/types/token_wrapper";
import {
  TOKEN_2022_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
  createMint,
  MINT_SIZE,
  getMinimumBalanceForRentExemptMint,
  createInitializeMint2Instruction,
  createMintToInstruction,
  createAssociatedTokenAccount,
  createAssociatedTokenAccountInstruction,
  getAssociatedTokenAddressSync,
  createTransferInstruction,
} from "@solana/spl-token";
import {
  Transaction,
  Message,
  VersionedTransaction,
  MessageV0,
  SystemProgram,
  Keypair,
  PublicKey,
  Connection,
  AccountMeta,
  sendAndConfirmTransaction,
  AccountInfo,
  RpcResponseAndContext,
  SimulatedTransactionResponse,
  TransactionInstruction,
} from "@solana/web3.js";
import { base64 } from "@coral-xyz/anchor/dist/cjs/utils/bytes";

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
  let numBytes = data.slice(4, 8);
  let numMetas = new anchor.BN(numBytes, null, "le");

  let realAccountMetas: AccountMeta[] = [];
  const metaSize = 34;
  for (let i = 0; i < numMetas.toNumber(); i += 1) {
    const start = 8 + i * metaSize;
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
      const preflightInstruction = await iProgram.methods
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
      let keys = await resolveRemainingAccounts(
        wrapper,
        await wrapper.provider.connection.simulateTransaction(transaction)
      );

      const tx = await iProgram.methods
        .transfer(new anchor.BN(1))
        .accounts({
          to: destination,
          owner: wallet,
          authority: wallet,
          mint: iProgram.programId,
        })
        .remainingAccounts(keys)
        .rpc();
      console.log("Transferred iProgram with wrapper", tx);
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

      let message = MessageV0.compile({
        payerKey: wallet,
        instructions: [...preInstructions, preflightInstruction],
        recentBlockhash: (
          await wrapper.provider.connection.getRecentBlockhash()
        ).blockhash,
      });
      let transaction = new VersionedTransaction(message);
      let keys = await resolveRemainingAccounts(
        wrapper,
        await wrapper.provider.connection.simulateTransaction(transaction)
      );

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
      message = MessageV0.compile({
        payerKey: wallet,
        instructions: [...preInstructions, instruction],
        recentBlockhash: (
          await wrapper.provider.connection.getRecentBlockhash()
        ).blockhash,
      });
      transaction = new VersionedTransaction(message);
      let tx = await wrapper.provider.sendAndConfirm(transaction, [], {
        skipPreflight: true,
      });

      console.log("Transferred spl token with wrapper", tx);
    });
  });
});
