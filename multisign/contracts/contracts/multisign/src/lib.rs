#![no_std]

mod test;

use soroban_sdk::{contract, contractimpl, contracterror, Address, BytesN, Env, Vec, Symbol, Map};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum MultisigError {
    Unauthorized = 1,
    InvalidThreshold = 2,
    TransactionNotFound = 3,
    TransactionExecuted = 4,
    AlreadyApproved = 5,
    InsufficientApprovals = 6,
    InvalidOwner = 7,
    ArithmeticError = 8,
    DuplicateOwner = 9,
}

#[contract]
pub struct MultisigContract;

#[derive(Clone, Debug)]
pub struct Transaction {
    pub to: Address,
    pub amount: i128,
    pub data: BytesN<32>,
    pub executed: bool,
    pub approvals: u32,
}

#[derive(Clone, Debug)]
pub struct MultisigConfig {
    pub owners: Vec<Address>,
    pub required_approvals: u32,
    pub transaction_count: u32,
}

const CONFIG_KEY: &Symbol = &Symbol::new("config");
const TX_KEY: &Symbol = &Symbol::new("tx");
const APPROVAL_KEY: &Symbol = &Symbol::new("approval");

#[contractimpl]
impl MultisigContract {
    pub fn initialize(
        env: Env,
        owners: Vec<Address>,
        required_approvals: u32,
    ) -> Result<(), MultisigError> {

        if env.storage().persistent().has(CONFIG_KEY) {
            panic!("Contract already initialized");
        }

        if owners.is_empty() {
            return Err(MultisigError::InvalidOwner);
        }
        
        if required_approvals == 0 || required_approvals > owners.len() as u32 {
            return Err(MultisigError::InvalidThreshold);
        }

        // Check for duplicate owners
        let mut seen = Map::new(&env);
        for owner in owners.iter() {
            if seen.contains_key(owner) {
                return Err(MultisigError::DuplicateOwner);
            }
            seen.set(owner, true);
        }

        let config = MultisigConfig {
            owners: owners.clone(),
            required_approvals,
            transaction_count: 0,
        };

        env.storage().persistent().set(CONFIG_KEY, &config);
        Ok(())
    }

    fn only_owner(env: &Env) -> Result<(), MultisigError> {
        let caller = env.invoker();
        let config: MultisigConfig = env.storage().persistent().get(CONFIG_KEY)
            .ok_or(MultisigError::Unauthorized)?;
        
        if !config.owners.contains(&caller) {
            return Err(MultisigError::Unauthorized);
        }
        
        Ok(())
    }

    fn get_config(env: &Env) -> Result<MultisigConfig, MultisigError> {
        env.storage().persistent().get(CONFIG_KEY)
            .ok_or(MultisigError::Unauthorized)
    }

    pub fn submit_transaction(
        env: Env,
        to: Address,
        amount: i128,
        data: BytesN<32>,
    ) -> Result<u32, MultisigError> {
        // Authentication check
        Address::require_auth(&env.invoker());
        Self::only_owner(&env)?;
        
        let mut config = Self::get_config(&env)?;
        
        let new_count = config.transaction_count.checked_add(1)
            .ok_or(MultisigError::ArithmeticError)?;
        
        let transaction = Transaction {
            to,
            amount,
            data,
            executed: false,
            approvals: 1, // Submitter auto-approves
        };

        // Update state BEFORE any external interactions
        config.transaction_count = new_count;
        env.storage().persistent().set(CONFIG_KEY, &config);
        
        // Store transaction
        env.storage().persistent().set(&(TX_KEY, new_count), &transaction);
        
        // Store approval for submitter
        let mut approvals = Vec::new(&env);
        approvals.push_back(env.invoker());
        env.storage().persistent().set(&(APPROVAL_KEY, new_count), &approvals);

        Ok(new_count)
    }

    pub fn approve_transaction(env: Env, transaction_id: u32) -> Result<(), MultisigError> {
        // Authentication check
        Address::require_auth(&env.invoker());
        Self::only_owner(&env)?;

        let tx_key = &(TX_KEY, transaction_id);
        let mut transaction: Transaction = env.storage().persistent().get(tx_key)
            .ok_or(MultisigError::TransactionNotFound)?;

        if transaction.executed {
            return Err(MultisigError::TransactionExecuted);
        }

        // Check if already approved
        let approval_key = &(APPROVAL_KEY, transaction_id);
        let mut approvals: Vec<Address> = env.storage().persistent().get(approval_key)
            .unwrap_or_else(|| Vec::new(&env));

        if approvals.contains(&env.invoker()) {
            return Err(MultisigError::AlreadyApproved);
        }

        // Safe arithmetic for approvals count
        let new_approvals = transaction.approvals.checked_add(1)
            .ok_or(MultisigError::ArithmeticError)?;
        
        // Update state BEFORE any external interactions
        transaction.approvals = new_approvals;
        env.storage().persistent().set(tx_key, &transaction);
        
        approvals.push_back(env.invoker());
        env.storage().persistent().set(approval_key, &approvals);

        Ok(())
    }

}