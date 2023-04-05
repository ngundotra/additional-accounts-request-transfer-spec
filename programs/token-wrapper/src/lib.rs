use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    bpf_loader::ID as BPF_LOADER_ID, bpf_loader_upgradeable::ID as BPF_UPGRADEABLE_LOADER,
};
use anchor_spl::{token::ID as TOKEN_PROGRAM_ID, token_2022::ID as TOKEN_PROGRAM22_ID};
use token_interface::{call, call_preflight_interface_function, IAccountMeta, PreflightPayload};

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod token_wrapper {
    use anchor_lang::solana_program::program::get_return_data;
    use anchor_spl::associated_token::get_associated_token_address;

    use super::*;

    pub fn preflight_transfer(ctx: Context<ITransfer>, amount: u64) -> Result<Vec<u8>> {
        let mint = &ctx.accounts.mint;
        if *mint.owner == TOKEN_PROGRAM22_ID || *mint.owner == TOKEN_PROGRAM_ID {
            // TOKEN invoke
            let source_ata = get_associated_token_address(ctx.accounts.owner.key, mint.key);
            let destination_ata = get_associated_token_address(ctx.accounts.to.key, mint.key);

            Ok(PreflightPayload {
                accounts: vec![
                    IAccountMeta {
                        pubkey: source_ata,
                        signer: false,
                        writable: true,
                    },
                    IAccountMeta {
                        pubkey: destination_ata,
                        signer: false,
                        writable: true,
                    },
                ],
            }
            .try_to_vec()?)
        } else if *mint.owner == BPF_LOADER_ID || *mint.owner == BPF_UPGRADEABLE_LOADER {
            // // Interface invoke
            // let program = mint;

            // // call the preflight function on the program
            // call_preflight_interface_function("transfer".to_string(), ctx)?;
            // let (pubkey, data) = get_return_data().unwrap();
            // assert_eq!(pubkey, program.key);
            Ok(vec![])
        } else {
            // Bad invoke
            return Err(ErrorCode::InstructionMissing.into());
        }
    }

    pub fn transfer(ctx: Context<TITransfer>, amount: u64) -> Result<()> {
        // call("transfer".to_string(), ctx, true)?;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct TITransfer<'info> {
    /// CHECK:
    pub owner: AccountInfo<'info>,
    /// CHECK:
    pub to: AccountInfo<'info>,
    pub authority: Signer<'info>,
    /// CHECK:
    pub mint: AccountInfo<'info>,
    /// CHECK:
    pub program: AccountInfo<'info>,
}

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
pub struct TIBurn<'info> {
    /// CHECK:
    pub owner: AccountInfo<'info>,
    pub authority: Signer<'info>,
    /// CHECK:
    pub mint: AccountInfo<'info>,
    /// CHECK:
    pub program: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct TIFreeze<'info> {
    /// CHECK:
    pub owner: AccountInfo<'info>,
    pub authority: Signer<'info>,
    /// CHECK:
    pub mint: AccountInfo<'info>,
    /// CHECK:
    pub program: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct TISetAuthority<'info> {
    /// CHECK:
    pub owner: AccountInfo<'info>,
    pub authority: Signer<'info>,
    /// CHECK:
    pub new_authority: AccountInfo<'info>,
    /// CHECK:
    pub mint: AccountInfo<'info>,
    /// CHECK:
    pub program: AccountInfo<'info>,
}
