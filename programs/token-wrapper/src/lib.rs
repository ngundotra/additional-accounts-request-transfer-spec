use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    bpf_loader::ID as BPF_LOADER_ID, bpf_loader_upgradeable::ID as BPF_UPGRADEABLE_LOADER,
    program::MAX_RETURN_DATA, sysvar::instructions::ID as SYSVAR_INSTRUCTIONS_ID,
};
use anchor_spl::{token::ID as TOKEN_PROGRAM_ID, token_2022::ID as TOKEN_PROGRAM22_ID};
use borsh::ser::BorshSerialize;
use mpl_token_metadata::{state::TokenMetadataAccount, ID as TOKEN_METADATA_ID};
use token_interface::{
    call, call_preflight_interface_function, IAccountMeta, ITransfer as _ITransfer,
    PreflightPayload,
};

declare_id!("F96CHxPDRgjUypdUqpJocgT59vEPT79AFJXjtyPCBaRt");

// This program is a wrapper around the token and token22 programs, in order to make
// them compliant with the `transfer` interface.
// It is also a pass-through interface to programs that adhere to the `transfer` interface.
// This means that you can use this program to `transfer` over both interface programs and token-* programs.
#[program]
pub mod token_wrapper {
    use anchor_lang::solana_program::{
        program::{get_return_data, set_return_data},
        system_program,
    };
    use anchor_spl::associated_token::{self, get_associated_token_address};
    use mpl_token_metadata::{
        assertions::metadata,
        pda::{find_master_edition_account, find_token_record_account},
        state::{Metadata, ProgrammableConfig},
    };

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
            CalleeProgram::TokenMetadata => {
                let meta = Metadata::from_account_info(&mint.to_account_info())?;

                let mint_address = meta.mint;
                let owner_ata = get_associated_token_address(ctx.accounts.owner.key, &mint_address);
                let destination_ata =
                    get_associated_token_address(ctx.accounts.to.key, &mint_address);
                let master_edition_address = Pubkey::find_program_address(
                    &[
                        "metadata".as_bytes(),
                        mpl_token_metadata::id().as_ref(),
                        mint_address.as_ref(),
                        "edition".as_bytes(),
                    ],
                    &mpl_token_metadata::id(),
                )
                .0;

                let owner_token_record = find_token_record_account(&mint_address, &owner_ata).0;
                let destination_token_record =
                    find_token_record_account(&mint_address, &destination_ata).0;

                let mut accounts: Vec<IAccountMeta> = vec![
                    // token-metadata
                    IAccountMeta {
                        pubkey: mpl_token_metadata::id(),
                        signer: false,
                        writable: false,
                    },
                    // #[account(0, writable, name="token", desc="Token account")]
                    IAccountMeta {
                        pubkey: owner_ata.key(),
                        signer: false,
                        writable: true,
                    },
                    // #[account(2, writable, name="destination", desc="Destination token account")]
                    IAccountMeta {
                        pubkey: destination_ata.key(),
                        signer: false,
                        writable: true,
                    },
                    // #[account(4, name="mint", desc="Mint of token asset")]
                    IAccountMeta {
                        pubkey: mint_address,
                        signer: false,
                        writable: false,
                    },
                    // #[account(6, optional, name="edition", desc="Edition of token asset")]
                    IAccountMeta {
                        pubkey: master_edition_address,
                        signer: false,
                        writable: false,
                    },
                    // #[account(7, optional, writable, name="owner_token_record", desc="Owner token record account")]
                    IAccountMeta {
                        pubkey: owner_token_record,
                        signer: false,
                        writable: true,
                    },
                    // #[account(8, optional, writable, name="destination_token_record", desc="Destination token record account")]
                    IAccountMeta {
                        pubkey: destination_token_record,
                        signer: false,
                        writable: true,
                    },
                    // #[account(11, name="system_program", desc="System Program")]
                    IAccountMeta {
                        pubkey: system_program::id(),
                        writable: false,
                        signer: false,
                    },
                    // #[account(12, name="sysvar_instructions", desc="Instructions sysvar account")]
                    IAccountMeta {
                        pubkey: SYSVAR_INSTRUCTIONS_ID,
                        signer: false,
                        writable: false,
                    },
                    // #[account(13, name="spl_token_program", desc="SPL Token Program")]
                    IAccountMeta {
                        pubkey: TOKEN_PROGRAM_ID,
                        signer: false,
                        writable: false,
                    },
                    // #[account(14, name="spl_ata_program", desc="SPL Associated Token Account program")]
                    IAccountMeta {
                        pubkey: associated_token::ID,
                        signer: false,
                        writable: false,
                    },
                ];

                match meta.programmable_config {
                    Some(programmable_config) => match programmable_config {
                        ProgrammableConfig::V1 { rule_set } => match rule_set {
                            Some(rule_set_pubkey) => {
                                msg!("Ruleset found: {}", rule_set_pubkey);
                                // #[account(15, optional, name="authorization_rules_program", desc="Token Authorization Rules Program")]
                                accounts.push(IAccountMeta {
                                    pubkey: mpl_token_auth_rules::ID,
                                    signer: false,
                                    writable: false,
                                });
                                // #[account(16, optional, name="authorization_rules", desc="Token Authorization Rules account")]
                                accounts.push(IAccountMeta {
                                    pubkey: rule_set_pubkey,
                                    signer: false,
                                    writable: false,
                                });
                            }
                            None => {
                                msg!("No programmable config found")
                            }
                        },
                    },
                    None => {
                        msg!("No programmable config found")
                    }
                }

                let serialized = PreflightPayload { accounts }.try_to_vec()?;
                msg!("Serialized len: {}, {}", serialized.len(), MAX_RETURN_DATA);
                set_return_data(&serialized);
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
            CalleeProgram::TokenMetadata => {
                // TokenMetadata invoke
                msg!("TokenMetadata");

                // Note: The first account is actually the token-metadata program account info
                let program_id = ctx.remaining_accounts.get(0).unwrap().key;
                let mut accounts: Vec<AccountInfo> = vec![];

                // yeah yeah it's inefficient, i know
                accounts.extend_from_slice(&ctx.remaining_accounts[1..]);

                // #[account(0, writable, name="token", desc="Token account")]
                // #[account(1, name="token_owner", desc="Token account owner")]
                accounts.insert(1, ctx.accounts.owner.to_account_info());
                // #[account(2, writable, name="destination", desc="Destination token account")]
                // #[account(3, name="destination_owner", desc="Destination token account owner")]
                accounts.insert(3, ctx.accounts.to.to_account_info());
                // #[account(4, name="mint", desc="Mint of token asset")]
                // #[account(5, writable, name="metadata", desc="Metadata (pda of ['metadata', program id, mint id])")]
                accounts.insert(5, ctx.accounts.mint.to_account_info());
                // #[account(6, optional, name="edition", desc="Edition of token asset")]
                // #[account(7, optional, writable, name="owner_token_record", desc="Owner token record account")]
                // #[account(8, optional, writable, name="destination_token_record", desc="Destination token record account")]
                // #[account(9, signer, name="authority", desc="Transfer authority (token owner or delegate)")]
                accounts.insert(9, ctx.accounts.authority.to_account_info());
                // #[account(10, signer, writable, name="payer", desc="Payer")]
                // yeah the authority has to be the payer here...
                // FIXME(ngundotra): this feels implicit, that all signers have to be the authority
                accounts.insert(10, ctx.accounts.authority.to_account_info());
                // #[account(11, name="system_program", desc="System Program")]
                // #[account(12, name="sysvar_instructions", desc="Instructions sysvar account")]
                // #[account(13, name="spl_token_program", desc="SPL Token Program")]
                // #[account(14, name="spl_ata_program", desc="SPL Associated Token Account program")]
                // #[account(15, optional, name="authorization_rules_program", desc="Token Authorization Rules Program")]
                // #[account(16, optional, name="authorization_rules", desc="Token Authorization Rules account")]

                let mut ix_data = vec![49, 0];
                ix_data.extend_from_slice(&amount.to_le_bytes());
                ix_data.push(0);
                let ix = anchor_lang::solana_program::instruction::Instruction {
                    program_id: *program_id,
                    data: ix_data,
                    accounts: accounts.to_account_metas(None),
                };
                anchor_lang::solana_program::program::invoke(&ix, accounts.as_slice())?;
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
    TokenMetadata,
    Error,
}

// This is a check that allows us to determine if we are calling
// an SPL token program, or a custom implementation
fn match_callee(mint_owner: &Pubkey) -> CalleeProgram {
    if *mint_owner == TOKEN_PROGRAM22_ID || *mint_owner == TOKEN_PROGRAM_ID {
        CalleeProgram::TokenStar
    } else if *mint_owner == BPF_LOADER_ID || *mint_owner == BPF_UPGRADEABLE_LOADER {
        // If the `mint` account is actually a program, then we know to call a custom program
        CalleeProgram::Interface
    } else if *mint_owner == TOKEN_METADATA_ID {
        CalleeProgram::TokenMetadata
    } else {
        // Here, we could add support for custom implementations of token programs
        CalleeProgram::Error
    }
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

// This is a copy-paste from `token-interface` crate, needed
// to make sure that we can deserialize the return data in
// our typescript client
#[derive(Debug, Clone, AnchorSerialize, AnchorDeserialize)]
pub struct ExternalIAccountMeta {
    pub pubkey: Pubkey,
    pub signer: bool,
    pub writable: bool,
}
