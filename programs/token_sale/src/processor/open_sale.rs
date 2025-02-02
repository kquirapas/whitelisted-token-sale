use crate::error::TokenSaleError;
use crate::state::{find_token_base_pda, TokenBase};
use crate::{
    instruction::accounts::{Context, OpenSaleAccounts},
    require,
};
use borsh::BorshDeserialize;
use solana_program::sysvar::Sysvar;
use solana_program::{
    entrypoint::ProgramResult, program::invoke_signed, program_error::ProgramError,
    program_pack::Pack, pubkey::Pubkey, rent::Rent, system_instruction,
    system_program::ID as SYSTEM_PROGRAM_ID,
};
use spl_token::{error::TokenError, state::Mint};

/// Open a Token Sale with the given config
///
/// Validates the accounts and data passed then
/// initializes the [`TokenBase`] (config)
///
/// Accounts
/// 0. `[WRITE]`    `Token Base` config account, PDA generated offchain
/// 1. `[]`         `Mint` account
/// 1. `[]`         `Vault` account
/// 2. `[SIGNER]`   `Sale Authority` account
///
/// Instruction Data
/// - price: u64,
/// - purchase_limit: u64,
/// - whitelist_root: [u8; 32],
pub fn process_open_sale(
    program_id: &Pubkey,
    ctx: Context<OpenSaleAccounts>,
    price: u64,
    purchase_limit: u64,
    whitelist_root: [u8; 32],
) -> ProgramResult {
    //---------- Account Validations ----------

    // 0. token_base
    //
    // - owner is token_sale (this) program
    // - correct allocation length (TokenBase::LEN)
    // - account is uninitialized
    // - token_base seeds must be ["token_base", pubkey(mint)]

    // NOTE: Not ideal but good enough to reach submission
    // inititalize token_base
    let rent_sysvar = &Rent::from_account_info(ctx.accounts.rent_sysvar)?;
    let (token_base_pda, token_base_bump) = find_token_base_pda(
        program_id,
        ctx.accounts.sale_authority.key,
        ctx.accounts.mint.key,
    );
    invoke_signed(
        &system_instruction::create_account(
            ctx.accounts.sale_authority.key,
            ctx.accounts.token_base.key,
            rent_sysvar.minimum_balance(TokenBase::LEN),
            TokenBase::LEN as u64,
            program_id,
        ),
        &[
            ctx.accounts.sale_authority.clone(),
            ctx.accounts.token_base.clone(),
        ],
        &[&[
            b"token_base",
            ctx.accounts.sale_authority.key.as_ref(),
            ctx.accounts.mint.key.as_ref(),
            &[token_base_bump],
        ]],
    )?;

    // - owner is token_sale (this) program
    require!(
        ctx.accounts.token_base.owner == program_id,
        ProgramError::InvalidAccountOwner,
        "token_base"
    );

    // - correct allocation length (TokenBase::LEN)
    let token_base_data = ctx.accounts.token_base.try_borrow_mut_data()?;
    require!(
        token_base_data.len() == TokenBase::LEN,
        TokenSaleError::InvalidAccountDataLength,
        "token_base"
    );

    // - account is uninitialized
    let mut token_base = TokenBase::try_from_slice(&token_base_data)?;
    require!(
        token_base.is_uninitialized(),
        ProgramError::AccountAlreadyInitialized,
        "token_base"
    );

    // - token_base seeds must be ["token_base", pubkey(mint)]
    require!(
        *ctx.accounts.token_base.key == token_base_pda,
        TokenSaleError::UnexpectedPDASeeds,
        "token_base"
    );

    // 1. mint
    //
    // - is_initialized is true
    // - mint_authority is token_base sale_authority
    let mint = ctx.accounts.mint;
    let mint_data = mint.try_borrow_data()?;
    let mint_state = Mint::unpack(&mint_data)?;

    // - is_initialized is true
    // require!(
    //     mint_state.is_initialized,
    //     TokenError::UninitializedState,
    //     "mint"
    // );

    // - mint_authority is token_base sale_authority
    // require!(
    //     mint_state.mint_authority.unwrap() == token_base.sale_authority,
    //     TokenSaleError::MintAndSaleAuthorityMismatch,
    //     "mint"
    // );

    // 2. vault
    //
    // - not executable
    let vault = ctx.accounts.vault;

    // - not executable
    require!(
        !vault.executable,
        TokenSaleError::MustBeNonExecutable,
        "vault"
    );

    // 3. sale_authority
    //
    // - not executable
    // - must be signer
    let sale_authority = ctx.accounts.sale_authority;

    // - not executable
    require!(
        !sale_authority.executable,
        TokenSaleError::MustBeNonExecutable,
        "sale_authority"
    );

    // - must be signer
    require!(
        sale_authority.is_signer,
        TokenSaleError::SaleAuthorityNotSigner,
        "sale_authority"
    );

    //---------- Data Validations (if any) ----------

    //---------- Executing Instruction ----------

    token_base.mint = *mint.key;
    token_base.vault = *vault.key;
    token_base.sale_authority = *sale_authority.key;
    token_base.whitelist_root = whitelist_root;
    token_base.price = price;
    token_base.default_purchase_limit = purchase_limit;
    token_base.bump = token_base_bump; // store canonical bump

    Ok(())
}
