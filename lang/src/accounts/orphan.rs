use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    ops::{Deref, DerefMut},
};

use anchor_lang::{prelude::*, system_program, AccountsClose};

use crate::bpf_writer::BpfWriter;

#[derive(Clone)]
pub struct OrphanAccount<'info, T: AccountSerialize + AccountDeserialize + Clone + Orphan> {
    account: T,
    info: AccountInfo<'info>,
}

impl<'info, T: AccountSerialize + AccountDeserialize + Clone + fmt::Debug + Orphan> fmt::Debug
    for OrphanAccount<'info, T>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OrphanAccount")
            .field("account", &self.account)
            .field("info", &self.info)
            .finish()
    }
}

impl<'a, T: AccountSerialize + AccountDeserialize + Clone + Orphan> OrphanAccount<'a, T> {
    fn new(info: AccountInfo<'a>, account: T) -> OrphanAccount<'a, T> {
        Self { info, account }
    }

    /// Deserializes the given `info` into a `Account`.
    #[inline(never)]
    pub fn try_from(info: &AccountInfo<'a>) -> Result<OrphanAccount<'a, T>> {
        if info.owner == &system_program::ID && info.lamports() == 0 {
            return Err(ErrorCode::AccountNotInitialized.into());
        }
        let mut data: &[u8] = &info.try_borrow_data()?;
        Ok(OrphanAccount::new(
            info.clone(),
            T::try_deserialize(&mut data)?,
        ))
    }

    /// Deserializes the given `info` into a `Account` without checking
    /// the account discriminator. Be careful when using this and avoid it if
    /// possible.
    #[inline(never)]
    pub fn try_from_unchecked(info: &AccountInfo<'a>) -> Result<OrphanAccount<'a, T>> {
        if info.owner == &system_program::ID && info.lamports() == 0 {
            return Err(ErrorCode::AccountNotInitialized.into());
        }
        let mut data: &[u8] = &info.try_borrow_data()?;
        Ok(OrphanAccount::new(
            info.clone(),
            T::try_deserialize_unchecked(&mut data)?,
        ))
    }

    /// Reloads the account from storage. This is useful, for example, when
    /// observing side effects after CPI.
    pub fn reload(&mut self) -> Result<()> {
        let mut data: &[u8] = &self.info.try_borrow_data()?;
        self.account = T::try_deserialize(&mut data)?;
        Ok(())
    }

    pub fn into_inner(self) -> T {
        self.account
    }

    pub fn set_inner(&mut self, inner: T) {
        self.account = inner;
    }
}

impl<'info, T: AccountSerialize + AccountDeserialize + Clone + Orphan> Accounts<'info>
    for OrphanAccount<'info, T>
where
    T: AccountSerialize + AccountDeserialize + Clone,
{
    #[inline(never)]
    fn try_accounts(
        _program_id: &Pubkey,
        accounts: &mut &[AccountInfo<'info>],
        _ix_data: &[u8],
        _bumps: &mut BTreeMap<String, u8>,
        _reallocs: &mut BTreeSet<Pubkey>,
    ) -> Result<Self> {
        if accounts.is_empty() {
            return Err(ErrorCode::AccountNotEnoughKeys.into());
        }
        let account = &accounts[0];
        *accounts = &accounts[1..];
        OrphanAccount::try_from(account)
    }
}

impl<'info, T: AccountSerialize + AccountDeserialize + Clone + Orphan> AccountsExit<'info>
    for OrphanAccount<'info, T>
{
    fn exit(&self, program_id: &Pubkey) -> Result<()> {
        // Only persist if the owner is the current program.
        if self.info.owner == program_id {
            let info = self.to_account_info();
            let mut data = info.try_borrow_mut_data()?;
            let dst: &mut [u8] = &mut data;
            let mut writer = BpfWriter::new(dst);
            self.account.try_serialize(&mut writer)?;
        }
        Ok(())
    }
}

/// This function is for INTERNAL USE ONLY.
/// Do NOT use this function in a program.
/// Manual closing of `OrphanAccount<'info, T>` types is NOT supported.
///
/// Details: Using `close` with `OrphanAccount<'info, T>` is not safe because
/// it requires the `mut` constraint but for that type the constraint
/// overwrites the "closed account" discriminator at the end of the instruction.
impl<'info, T: AccountSerialize + AccountDeserialize + Clone + Orphan> AccountsClose<'info>
    for OrphanAccount<'info, T>
{
    fn close(&self, sol_destination: AccountInfo<'info>) -> Result<()> {
        crate::common::close(self.to_account_info(), sol_destination)
    }
}

impl<'info, T: AccountSerialize + AccountDeserialize + Clone + Orphan> ToAccountMetas
    for OrphanAccount<'info, T>
{
    fn to_account_metas(&self, is_signer: Option<bool>) -> Vec<AccountMeta> {
        let is_signer = is_signer.unwrap_or(self.info.is_signer);
        let meta = match self.info.is_writable {
            false => AccountMeta::new_readonly(*self.info.key, is_signer),
            true => AccountMeta::new(*self.info.key, is_signer),
        };
        vec![meta]
    }
}

impl<'info, T: AccountSerialize + AccountDeserialize + Clone + Orphan> ToAccountInfos<'info>
    for OrphanAccount<'info, T>
{
    fn to_account_infos(&self) -> Vec<AccountInfo<'info>> {
        vec![self.info.clone()]
    }
}

pub trait Orphan {}

impl<'info, T: AccountSerialize + AccountDeserialize + Clone + Orphan> AsRef<AccountInfo<'info>>
    for OrphanAccount<'info, T>
{
    fn as_ref(&self) -> &AccountInfo<'info> {
        &self.info
    }
}

impl<'info, T: AccountSerialize + AccountDeserialize + Clone + Orphan> AsRef<T>
    for OrphanAccount<'info, T>
{
    fn as_ref(&self) -> &T {
        &self.account
    }
}

impl<'a, T: AccountSerialize + AccountDeserialize + Clone + Orphan> Deref for OrphanAccount<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &(*self).account
    }
}

impl<'a, T: AccountSerialize + AccountDeserialize + Clone + Orphan> DerefMut
    for OrphanAccount<'a, T>
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        #[cfg(feature = "anchor-debug")]
        if !self.info.is_writable {
            solana_program::msg!("The given Account is not mutable");
            panic!();
        }
        &mut self.account
    }
}

impl<'info, T: AccountSerialize + AccountDeserialize + Clone + Orphan> Key
    for OrphanAccount<'info, T>
{
    fn key(&self) -> Pubkey {
        *self.info.key
    }
}
