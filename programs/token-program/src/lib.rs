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
    use anchor_lang::solana_program::program::{get_return_data, invoke, set_return_data};
    use anchor_lang::solana_program::{hash, instruction::Instruction};
    use token_interface::{IAccountMeta, PreflightPayload};

    use super::*;

    /// Just used to initialize this program's singleton Ledger account
    pub fn initialize_mint(ctx: Context<InitializeMint>, supply: u64) -> Result<()> {
        let ledger = &mut ctx.accounts.ledger;
        let authority = &ctx.accounts.authority.key();
        let mut accounts: LedgerAccounts = HashMap::with_capacity(DEFAULT_CAPACITY);
        ledger.mint_authority = *authority;
        ledger.freeze_authority = *authority;
        ledger.total_supply = supply;

        // Hack to avoid creating `mint_to` instruction
        // just mint all supply to the authority
        accounts.insert(
            *authority,
            LedgerAccount {
                amount: supply,
                is_frozen: false,
            },
        );
        ledger.opaque_accounts = accounts.try_to_vec().unwrap();
        Ok(())
    }

    pub fn preflight_transfer(ctx: Context<ITransfer>, amount: u64) -> Result<()> {
        let ledger = Pubkey::find_program_address(&[LEDGER_PREFIX.as_bytes()], &crate::id()).0;

        set_return_data(
            &PreflightPayload {
                accounts: vec![IAccountMeta {
                    pubkey: ledger,
                    signer: false,
                    writable: true,
                }],
            }
            .try_to_vec()
            .unwrap(),
        );
        Ok(())
    }

    // Transfer tokens from one account to another
    // but only update their stored balance in the ledger account
    pub fn transfer(ctx: Context<Transfer>, amount: u64) -> Result<()> {
        assert_eq!(ctx.accounts.authority.key(), ctx.accounts.owner.key());
        let ledger = &mut ctx.accounts.ledger;

        let mut accounts = get_ledger_accounts(&ledger.opaque_accounts)?;
        update_balance(&mut accounts, ctx.accounts.owner.key, amount, false)?;

        if accounts.get(&ctx.accounts.to.key()).is_none() {
            if accounts.len() >= DEFAULT_CAPACITY {
                return Err(TokenError::LedgerCapacityFull.into());
            } else {
                accounts.insert(
                    ctx.accounts.to.key(),
                    LedgerAccount {
                        amount: 0,
                        is_frozen: false,
                    },
                );
            }
        }

        update_balance(&mut accounts, ctx.accounts.to.key, amount, true)?;
        ledger.opaque_accounts = accounts.try_to_vec().unwrap();

        Ok(())
    }
}

type LedgerAccounts = HashMap<Pubkey, LedgerAccount>;
fn get_ledger_accounts(data: &[u8]) -> Result<LedgerAccounts> {
    Ok(LedgerAccounts::try_from_slice(&data)?)
}

fn update_balance(
    accounts: &mut LedgerAccounts,
    owner: &Pubkey,
    amount: u64,
    is_add: bool,
) -> Result<()> {
    let mut account = accounts.get(owner).unwrap().clone();
    if is_add {
        account.amount = account
            .amount
            .checked_add(amount)
            .ok_or(TokenError::MathOverflow)?;
    } else {
        account.amount = account
            .amount
            .checked_sub(amount)
            .ok_or(TokenError::InsufficientFunds)?;
    }
    accounts.insert(*owner, account);
    Ok(())
}

#[account]
pub struct Ledger {
    pub total_supply: u64,
    pub mint_authority: Pubkey,
    pub freeze_authority: Pubkey,
    // pub accounts: HashMap<Pubkey, u64>,
    pub opaque_accounts: Vec<u8>,
}

#[derive(Debug, Clone, AnchorSerialize, AnchorDeserialize)]
pub struct LedgerAccount {
    pub amount: u64,
    pub is_frozen: bool,
}

#[derive(Accounts)]
pub struct InitializeMint<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(init,
        seeds=[LEDGER_PREFIX.as_bytes()],
        bump,
        payer=authority,
        space=8 + 8 + 32 + 32 + 4 + DEFAULT_CAPACITY * (32 + std::mem::size_of::<LedgerAccount>())
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
    /// CHECK:
    pub mint: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct Transfer<'info> {
    /// CHECK:
    pub owner: AccountInfo<'info>,
    /// CHECK:
    pub to: AccountInfo<'info>,
    pub authority: Signer<'info>,
    /// CHECK:
    pub mint: AccountInfo<'info>,
    #[account(mut, seeds=[LEDGER_PREFIX.as_bytes()], bump)]
    pub ledger: Account<'info, Ledger>,
}
