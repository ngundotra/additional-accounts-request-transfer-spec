import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { ItokenPoc } from "../target/types/itoken_poc";
import { TokenProgram } from "../target/types/token_program";
import { publicKey } from "@coral-xyz/anchor/dist/cjs/utils";

describe("itoken-poc", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const wallet = anchor.getProvider().publicKey!;

  describe("'ITokenProgram'", () => {
    const program = anchor.workspace.TokenProgram as Program<TokenProgram>;

    it("Can initialize & transfer", async () => {
      // Add your test here.
      let tx = await program.methods
        .initializeMint(new anchor.BN(10))
        .accounts({
          authority: wallet,
        })
        .rpc();
      console.log("Initialized", tx);

      const destination = anchor.web3.Keypair.generate().publicKey;
      tx = await program.methods
        .transfer(new anchor.BN(5))
        .accounts({
          owner: wallet,
          authority: wallet,
          to: destination,
        })
        .rpc();
      console.log("Transferred", tx);
      tx = await program.methods
        .view("get_balance_of", destination.toString())
        .accounts({
          target: anchor.web3.PublicKey.findProgramAddressSync(
            [Buffer.from("ledger")],
            program.programId
          )[0],
        })
        .remainingAccounts([
          {
            pubkey: program.programId,
            isSigner: false,
            isWritable: false,
          },
        ])

        .rpc();
      console.log("Balance details", tx);
    });
  });
});
