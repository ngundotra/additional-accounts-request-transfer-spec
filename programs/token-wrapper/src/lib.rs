use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    bpf_loader::ID as BPF_LOADER_ID, bpf_loader_upgradeable::ID as BPF_UPGRADEABLE_LOADER,
};
use anchor_spl::{token::ID as TOKEN_PROGRAM_ID, token_2022::ID as TOKEN_PROGRAM22_ID};
use token_interface::{
    call, call_preflight_interface_function, IAccountMeta, ITransfer as _ITransfer,
    PreflightPayload,
};

declare_id!("F96CHxPDRgjUypdUqpJocgT59vEPT79AFJXjtyPCBaRt");

#[program]
pub mod token_wrapper {
    use anchor_lang::solana_program::program::{get_return_data, set_return_data};
    use anchor_spl::associated_token::get_associated_token_address;

    use super::*;

    pub fn preflight_transfer(ctx: Context<ITransfer>, amount: u64) -> Result<()> {
        let mint = &ctx.accounts.mint;
        match match_callee(mint.owner) {
            CalleeProgram::TokenStar => {
                // TOKEN invoke
                let source_ata = get_associated_token_address(ctx.accounts.owner.key, mint.key);
                let destination_ata = get_associated_token_address(ctx.accounts.to.key, mint.key);

                set_return_data(
                    &PreflightPayload {
                        accounts: vec![
                            IAccountMeta {
                                pubkey: *mint.owner,
                                signer: false,
                                writable: false,
                            },
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
                    .try_to_vec()?,
                );
                Ok(())
            }
            CalleeProgram::Interface => {
                // Interface invoke
                let ctx = CpiContext::new(
                    mint.to_account_info(),
                    _ITransfer {
                        to: ctx.accounts.to.to_account_info(),
                        owner: ctx.accounts.owner.to_account_info(),
                        authority: ctx.accounts.authority.clone(),
                        mint: mint.to_account_info(),
                    },
                );
                call_preflight_interface_function(
                    "transfer".to_string(),
                    &ctx,
                    &amount.try_to_vec()?,
                )?;
                let (key, return_data) = get_return_data().unwrap();
                assert_eq!(key, *mint.key);
                set_return_data(&return_data);
                Ok(())
            }
            // Bad invoke
            _ => return Err(ErrorCode::InstructionMissing.into()),
        }
    }

    pub fn transfer<'info>(
        ctx: Context<'_, '_, '_, 'info, ITransfer<'info>>,
        amount: u64,
    ) -> Result<()> {
        let mint = &ctx.accounts.mint;
        match match_callee(mint.owner) {
            CalleeProgram::TokenStar => {
                // Token invoke
                msg!("Token-*");
                let remaining_accounts = ctx.remaining_accounts;
                let token = remaining_accounts.get(0).unwrap().to_account_info();
                let from = remaining_accounts.get(1).unwrap().to_account_info();
                let to = remaining_accounts.get(2).unwrap().to_account_info();
                let ctx = CpiContext::new(
                    token,
                    anchor_spl::token_interface::TransferChecked {
                        to,
                        from,
                        mint: mint.to_account_info(),
                        authority: ctx.accounts.authority.to_account_info(),
                    },
                );

                let raw_mint_data = mint.data.try_borrow().map_err(|e| {
                    anchor_lang::solana_program::msg!("Failed to borrow mint data: {:?}", e);
                    ErrorCode::InstructionMissing
                })?;
                let mut ptr = raw_mint_data.as_ref();
                let mint_data = anchor_spl::token_interface::Mint::try_deserialize(&mut ptr)?;
                anchor_spl::token_interface::transfer_checked(ctx, amount, mint_data.decimals)?;
            }
            CalleeProgram::Interface => {
                // Interface invoke
                msg!("Interface");
                let ctx = CpiContext::new(
                    mint.to_account_info(),
                    _ITransfer {
                        to: ctx.accounts.to.to_account_info(),
                        owner: ctx.accounts.owner.to_account_info(),
                        authority: ctx.accounts.authority.clone(),
                        mint: mint.to_account_info(),
                    },
                )
                .with_remaining_accounts(ctx.remaining_accounts.to_vec());
                call("transfer".to_string(), ctx, amount.try_to_vec()?, false)?;
            }
            // Bad invoke
            _ => return Err(ErrorCode::InstructionMissing.into()),
        }
        Ok(())
    }
}

enum CalleeProgram {
    TokenStar,
    Interface,
    Error,
}

fn match_callee(mint_owner: &Pubkey) -> CalleeProgram {
    if *mint_owner == TOKEN_PROGRAM22_ID || *mint_owner == TOKEN_PROGRAM_ID {
        CalleeProgram::TokenStar
    } else if *mint_owner == BPF_LOADER_ID || *mint_owner == BPF_UPGRADEABLE_LOADER {
        CalleeProgram::Interface
    } else {
        CalleeProgram::Error
    }
}

// #[derive(Accounts)]
// pub struct TITransfer<'info> {
//     /// CHECK:
//     pub owner: AccountInfo<'info>,
//     /// CHECK:
//     pub to: AccountInfo<'info>,
//     pub authority: Signer<'info>,
//     /// CHECK:
//     pub mint: AccountInfo<'info>,
//     /// CHECK:
//     pub program: AccountInfo<'info>,
// }

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

#[derive(Debug, Clone, AnchorSerialize, AnchorDeserialize)]
pub struct ExternalIAccountMeta {
    pub pubkey: Pubkey,
    pub signer: bool,
    pub writable: bool,
}
