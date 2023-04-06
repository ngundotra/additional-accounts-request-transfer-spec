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
  SystemProgram,
  Keypair,
  PublicKey,
  sendAndConfirmTransaction,
} from "@solana/web3.js";

describe("itoken-poc", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const wallet = anchor.getProvider().publicKey!;

  describe("Token Wrapper", () => {
    const wrapper = anchor.workspace.TokenWrapper as Program<TokenWrapper>;
    const tokenkeg = TOKEN_PROGRAM_ID;
    const iProgram = anchor.workspace.TokenProgram as Program<TokenProgram>;

    // Tokenkeg
    let destination: PublicKey = Keypair.generate().publicKey;
    let tokenMint: PublicKey;
    let ata: PublicKey;
    let destinationAta: PublicKey;

    // iProgram
    let ledger: PublicKey = PublicKey.findProgramAddressSync(
      [Buffer.from("ledger")],
      iProgram.programId
    )[0];

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
      let tx = await iProgram.methods
        .transfer(new anchor.BN(1))
        .accounts({
          to: destination,
          owner: wallet,
          authority: wallet,
          mint: iProgram.programId,
        })
        .remainingAccounts([
          {
            pubkey: ledger,
            isSigner: false,
            isWritable: true,
          },
        ])
        .rpc();
      console.log("Transferred iProgram with wrapper", tx);
    });
    it("Can transfer tokenkeg using wrapper", async () => {
      let tx = await wrapper.methods
        .transfer(new anchor.BN(1))
        .accounts({
          to: destination,
          owner: wallet,
          mint: tokenMint,
          authority: wallet,
        })
        .remainingAccounts([
          {
            pubkey: tokenkeg,
            isSigner: false,
            isWritable: false,
          },
          {
            pubkey: ata,
            isSigner: false,
            isWritable: true,
          },
          {
            pubkey: destinationAta,
            isSigner: false,
            isWritable: true,
          },
        ])
        .preInstructions([
          createAssociatedTokenAccountInstruction(
            wallet,
            destinationAta,
            destination,
            tokenMint,
            tokenkeg
          ),
        ])
        .rpc({ skipPreflight: true });

      console.log("Transferred spl token with wrapper", tx);
    });
  });
});
