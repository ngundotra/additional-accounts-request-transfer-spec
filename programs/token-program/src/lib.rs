use anchor_lang::prelude::*;

declare_id!("32d7pdBNmDmXAXcHkivteqLjaNVovWQ1JMn85LdyyAux");
use std::collections::HashMap;

#[error_code]
pub enum TokenError {
    #[msg("Insufficient funds")]
    InsufficientFunds,
    #[msg("Math overflow")]
    MathOverflow,
    #[msg("Ledger capacity is full")]
    LedgerCapacityFull,
}

pub const LEDGER_PREFIX: &str = "ledger";
pub const DEFAULT_CAPACITY: usize = 5;

#[program]
pub mod token_program {
    use anchor_lang::solana_program::program::{get_return_data, invoke};
    use anchor_lang::solana_program::{hash, instruction::Instruction};
    use token_interface::{IAccountMeta, PreflightPayload};

    use super::*;

    /// TODO: add this to interface specification
    pub fn preflight_initialize_mint(ctx: Context<InitializeMint>, supply: u64) -> Result<()> {
        Ok(())
    }

    pub fn initialize_mint(ctx: Context<InitializeMint>, supply: u64) -> Result<()> {
        let ledger = &mut ctx.accounts.ledger;
        let authority = &ctx.accounts.authority.key();
        let mut accounts: HashMap<Pubkey, u64> = HashMap::with_capacity(DEFAULT_CAPACITY);
        ledger.mint_authority = *authority;
        ledger.freeze_authority = *authority;
        ledger.total_suppy = supply;

        // Hack to avoid creating `mint_to` instruction
        // just mint all supply to the authority
        accounts.insert(*authority, supply);
        ledger.opaque_accounts = accounts.try_to_vec().unwrap();

        Ok(())
    }

    pub fn preflight_transfer(ctx: Context<Transfer>, amount: u64) -> Result<Vec<u8>> {
        let ledger = Pubkey::find_program_address(&[LEDGER_PREFIX.as_bytes()], &crate::id()).0;
        Ok(PreflightPayload {
            accounts: vec![IAccountMeta {
                pubkey: ledger,
                signer: false,
                writable: true,
            }],
        }
        .try_to_vec()
        .unwrap())
    }

    pub fn transfer(ctx: Context<Transfer>, amount: u64) -> Result<()> {
        let ledger = &mut ctx.accounts.ledger;

        let mut accounts: HashMap<Pubkey, u64> =
            HashMap::<Pubkey, u64>::try_from_slice(&ledger.opaque_accounts)?;

        let mut owner_amt = *accounts.get(&ctx.accounts.owner.key()).unwrap();
        owner_amt = owner_amt
            .checked_sub(amount)
            .ok_or(TokenError::InsufficientFunds)?;
        accounts.insert(ctx.accounts.owner.key(), owner_amt);

        if accounts.get(&ctx.accounts.to.key()).is_none() {
            if accounts.len() >= DEFAULT_CAPACITY {
                return Err(TokenError::LedgerCapacityFull.into());
            } else {
                accounts.insert(ctx.accounts.to.key(), 0);
            }
        }

        let mut dest_amt = *accounts.get(&ctx.accounts.to.key()).unwrap();
        dest_amt = dest_amt
            .checked_add(amount)
            .ok_or(TokenError::MathOverflow)?;
        accounts.insert(ctx.accounts.to.key(), dest_amt);

        ledger.opaque_accounts = accounts.try_to_vec().unwrap();

        Ok(())
    }

    pub fn view(ctx: Context<ViewFunction>, function_name: String, args: String) -> Result<()> {
        msg!("view function: {}", function_name);

        let mut ix_data = hash::hash(format!("global:view_{}", function_name).as_bytes())
            .to_bytes()[..8]
            .to_vec();
        ix_data.extend_from_slice(&args.try_to_vec()?);

        invoke(
            &Instruction {
                program_id: crate::id(),
                accounts: ctx.accounts.to_account_metas(None),
                data: ix_data,
            },
            &ctx.accounts.to_account_infos(),
        )?;

        let (key, data) = get_return_data().unwrap();
        assert_eq!(key, crate::id());
        let result = String::try_from_slice(&data).unwrap();
        msg!("result: {}", result);

        Ok(())
    }

    pub fn view_get_balance_of(ctx: Context<GetBalanceOf>, owner_str: String) -> Result<String> {
        let ledger = &ctx.accounts.ledger;
        let owner = Pubkey::try_from_slice(bs58::decode(owner_str).into_vec().unwrap().as_slice())?;
        let accounts: HashMap<Pubkey, u64> =
            HashMap::<Pubkey, u64>::try_from_slice(&ledger.opaque_accounts)?;
        Ok(format!("{}", *accounts.get(&owner).unwrap_or(&0)))
    }
}

#[account]
pub struct Ledger {
    pub total_suppy: u64,
    pub mint_authority: Pubkey,
    pub freeze_authority: Pubkey,
    // pub accounts: HashMap<Pubkey, u64>,
    pub opaque_accounts: Vec<u8>,
}

#[derive(Debug, Clone, AnchorSerialize, AnchorDeserialize)]
pub struct LedgerAccount {
    pub key: Pubkey,
    pub amount: u64,
}

#[derive(Accounts)]
pub struct InitializeMint<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(init,
        seeds=[LEDGER_PREFIX.as_bytes()],
        bump,
        payer=authority,
        space=8 + std::mem::size_of::<Ledger>() + DEFAULT_CAPACITY * (std::mem::size_of::<Pubkey>() + 8)
    )]
    pub ledger: Account<'info, Ledger>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ITransfer<'info> {
    /// CHECK:
    pub owner: AccountInfo<'info>,
    /// CHECK:
    pub to: AccountInfo<'info>,
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct Transfer<'info> {
    /// CHECK:
    pub owner: AccountInfo<'info>,
    /// CHECK:
    pub to: AccountInfo<'info>,
    pub authority: Signer<'info>,
    #[account(mut, seeds=[LEDGER_PREFIX.as_bytes()], bump)]
    pub ledger: Account<'info, Ledger>,
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

#[derive(Accounts)]
pub struct ViewFunction<'info> {
    /// CHECK:
    target: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct GetBalanceOf<'info> {
    pub ledger: Account<'info, Ledger>,
}
