use anchor_lang::prelude::*;

pub trait ToTargetProgram<'info> {
    type TargetCtx<'_info>: ToAccountInfos<'_info> + ToAccountMetas;

    fn to_target_program(&self) -> Pubkey;
    fn get_target_program(&self) -> AccountInfo<'info>;
    fn to_target_context(
        &self,
        remaining_accounts: Vec<AccountInfo<'info>>,
    ) -> CpiContext<'_, '_, '_, 'info, Self::TargetCtx<'info>>;
}
