#![feature(generic_associated_types)]
use std::collections::HashMap;

pub mod traits;
use traits::*;

use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    hash,
    program::{get_return_data, invoke},
};

#[derive(Debug, Clone, AnchorSerialize, AnchorDeserialize)]
pub struct IAccountMeta {
    pub pubkey: Pubkey,
    pub signer: bool,
    pub writable: bool,
}

/// TODO:
/// - add discriminant
#[derive(Debug, Clone, AnchorDeserialize, AnchorSerialize)]
pub struct PreflightPayload {
    // pub discriminant: Vec<u8>,
    pub accounts: Vec<IAccountMeta>,
}

impl PreflightPayload {
    pub fn match_accounts<'info>(
        &self,
        accounts: &[AccountInfo<'info>],
    ) -> Result<Vec<AccountInfo<'info>>> {
        let mut map = HashMap::<Pubkey, AccountInfo>::new();

        for acc in accounts {
            map.insert(acc.key(), acc.clone());
        }

        let mut found_accounts = Vec::<AccountInfo>::new();
        for acc in self.accounts.iter() {
            let found_acc = map.get(&acc.pubkey);
            if found_acc.is_none() {
                msg!(&format!("account not found: {:?}", acc.pubkey));
                return Err(ProgramError::NotEnoughAccountKeys.into());
            }
            found_accounts.push(found_acc.unwrap().clone());
        }
        msg!("found accounts: {:?}", found_accounts.len());

        Ok(found_accounts)
    }
}

pub fn get_interface_accounts(program_key: &Pubkey) -> Result<PreflightPayload> {
    let (key, program_data) = get_return_data().unwrap();
    assert_eq!(key, *program_key);
    let program_data = program_data.as_slice();
    let additional_interface_accounts = PreflightPayload::try_from_slice(&program_data)?;
    msg!(
        "Additional interface accounts: {:?}",
        &additional_interface_accounts
    );
    Ok(additional_interface_accounts)
}

/// TODO:
///  - add support for invoking the target program directly (without the wrapper)
///  - add support for adding the ix discriminant
pub fn call<
    'info,
    C1: ToAccountInfos<'info> + ToAccountMetas + ToTargetProgram<'info, TargetCtx<'info> = C2>,
    C2: ToAccountInfos<'info> + ToAccountMetas,
>(
    ix_name: String,
    ctx: CpiContext<'_, '_, '_, 'info, C1>,
    args: Vec<u8>,
    log_info: bool,
) -> Result<()> {
    // preflight
    if log_info {
        msg!("Preflight");
    }
    call_preflight_interface_function(ix_name.clone(), &ctx, &args)?;

    // parse cpi return data
    if log_info {
        msg!("Parse return data");
    }
    let additional_interface_accounts = get_interface_accounts(&ctx.accounts.to_target_program())?;

    // wrap into target context
    if log_info {
        msg!("Convert into target context");
    }
    let cpi_ctx: CpiContext<C2> = ctx
        .accounts
        .to_target_context(ctx.remaining_accounts.to_vec());

    // execute
    if log_info {
        msg!("Execute {}", &ix_name);
    }
    call_interface_function(
        ix_name.clone(),
        cpi_ctx,
        &args,
        additional_interface_accounts,
        log_info,
    )?;
    Ok(())
}

pub fn call_preflight_interface_function<'info, T: ToAccountInfos<'info> + ToAccountMetas>(
    function_name: String,
    ctx: &CpiContext<'_, '_, '_, 'info, T>,
    args: &[u8],
) -> Result<()> {
    // setup
    let mut ix_data: Vec<u8> =
        hash::hash(format!("global:preflight_{}", &function_name).as_bytes()).to_bytes()[..8]
            .to_vec();

    ix_data.extend_from_slice(args);

    let ix_account_metas = ctx.accounts.to_account_metas(Some(false));
    let ix = anchor_lang::solana_program::instruction::Instruction {
        program_id: ctx.program.key(),
        accounts: ix_account_metas,
        data: ix_data,
    };

    // execute
    invoke(&ix, &ctx.accounts.to_account_infos())?;
    Ok(())
}

pub fn call_interface_function<'info, T: ToAccountInfos<'info> + ToAccountMetas>(
    function_name: String,
    ctx: CpiContext<'_, '_, '_, 'info, T>,
    args: &[u8],
    additional_interface_accounts: PreflightPayload,
    log_info: bool,
) -> Result<()> {
    // setup
    let remaining_accounts = ctx.remaining_accounts.to_vec();

    let mut ix_data: Vec<u8> =
        hash::hash(format!("global:{}", &function_name).as_bytes()).to_bytes()[..8].to_vec();
    ix_data.extend_from_slice(&args);

    let mut ix_account_metas = ctx.accounts.to_account_metas(None);
    ix_account_metas.append(
        additional_interface_accounts
            .accounts
            .iter()
            .map(|acc| {
                if acc.writable {
                    AccountMeta::new(acc.pubkey, acc.signer)
                } else {
                    AccountMeta::new_readonly(acc.pubkey, acc.signer)
                }
            })
            .collect::<Vec<AccountMeta>>()
            .as_mut(),
    );

    let ix = anchor_lang::solana_program::instruction::Instruction {
        program_id: ctx.program.key(),
        accounts: ix_account_metas,
        data: ix_data,
    };

    let mut ix_ais: Vec<AccountInfo> = ctx.accounts.to_account_infos();
    if log_info {
        msg!("IX accounts: {:?}", &ix_ais.len());
    }
    ix_ais.extend_from_slice(
        &mut additional_interface_accounts
            .match_accounts(&remaining_accounts)?
            .to_vec(),
    );

    if log_info {
        msg!("IX accounts: {:?}", &ix_ais.len());
    }

    if log_info {
        ix_ais.iter().into_iter().for_each(|ai| {
            msg!(
                "Account: {:?}, {:?}, {:?}, {:?}",
                ai.key,
                ai.owner,
                ai.is_signer,
                ai.is_writable
            )
        });
    } else {
        // execute
        invoke(&ix, &ix_ais)?;
    }
    Ok(())
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

// #[derive(Accounts)]
// pub struct TIUnlock<'info> {
//     #[account(mut)]
//     pub token: InterfaceAccount<'info, TokenAccount>,
//     pub mint: InterfaceAccount<'info, Mint>,
//     pub delegate: Signer<'info>,
//     pub token_program: Interface<'info, TokenInterface>,
//     /// CHECK: permission program
//     pub perm_program: AccountInfo<'info>,
//     // ix_accounts: Option<Account<'info, IxAccounts>>,
// }

// #[derive(Accounts)]
// pub struct IUnlock<'info> {
//     #[account(mut)]
//     pub token: AccountInfo<'info>,
//     pub mint: AccountInfo<'info>,
//     pub delegate: AccountInfo<'info>,
//     pub token_program: AccountInfo<'info>,
// }

impl<'info> ToTargetProgram<'info> for ITransfer<'info> {
    type TargetCtx<'a> = ITransfer<'a>;

    fn to_target_program(&self) -> Pubkey {
        self.mint.key()
    }
    fn get_target_program(&self) -> AccountInfo<'info> {
        self.mint.clone()
    }

    fn to_target_context(
        &self,
        remaining_accounts: Vec<AccountInfo<'info>>,
    ) -> CpiContext<'_, '_, '_, 'info, Self::TargetCtx<'info>> {
        let inner = ITransfer {
            to: self.to.to_account_info(),
            mint: self.mint.to_account_info(),
            owner: self.owner.to_account_info(),
            authority: self.authority.clone(),
        };
        CpiContext::new(self.get_target_program(), inner)
            .with_remaining_accounts(remaining_accounts)
    }
}
